//! ZMVP-66 — the owner hard-deletes a fact-free commission, end to end over HTTP.
//!
//! Pins the acceptance criteria at the API surface:
//!
//! - **AC1** — `DELETE /commissions/{id}` by the owner removes a fact-free
//!   commission entirely: the row is gone and its changelog cascades away with it
//!   (the mem backend mirrors the pg `ON DELETE CASCADE`; the pg cascade itself is
//!   proven in `adapter-pg/tests/commission.rs`);
//! - **AC2** — a non-owner cannot delete it: a caller who may not see the
//!   commission gets the **uniform 404** (`commission_not_found`, the closed-door
//!   policy) — identical for a hidden and a truly absent commission — and an
//!   anonymous caller gets `401`. (The participant-but-not-owner `403` arm of the
//!   shared `require_owner` seam is unreachable until ZMVP-79 seats non-owner
//!   participants.)
//! - **AC3** — deleting a fact-bearing commission is rejected with the `409`
//!   `commission_has_facts` problem whose detail points at Archive, and nothing is
//!   deleted. No fact-minter exists yet, so the fact-bearing state is staged with
//!   a test double at the [`Database`] port seam: it wraps the mem unit of work
//!   and answers `commission_has_facts` with `true`, proving the handler's gate
//!   consults the predicate **inside the unit of work that would delete** and
//!   refuses.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use async_trait::async_trait;
use chrono::Utc;
use domain::elements::{
    account::AccountId,
    commission::{
        ChannelPointer, Commission, CommissionId, CommissionTitle, GrantLevel, NewComponent,
        NewSurface, NodeId,
    },
    did::Did,
    maturity::Maturity,
    profile::Profile,
    user::UserId,
};
use domain::ports::{
    AccountWrites, ActorIdentityWrites, ChangelogWrites, CommissionWrites, Database, UnitOfWork,
    UserWrites,
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

/// Boots the app around a caller-supplied backend + database pair, so a test can
/// interpose at the unit-of-work seam while reads keep hitting the same backend.
/// `did` is the identity `sign_in` will authenticate as.
async fn spawn_app_on(did: &str, backend: &MemBackend, database: Arc<dyn Database>) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let state = AppState {
        config: Config {
            env: Environment::DEV,
            http_addr: addr,
            public_url: format!("http://{addr}"),
            database_url: "postgres://unused".to_string(),
            log_level: "info".to_string(),
            handle_domain: "zurfur.app".to_string(),
            did_key_root_key: "unused-in-tests".to_string(),
            plc_directory_endpoint: "https://plc.directory".to_string(),
            plc_directory_submit: false,
            deadline_sweep_interval_secs: 60,
            max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
        },
        files: backend.file_store(),
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new(did.to_string()))),
        users: backend.user_store(),
        profile_source: Arc::new(MemProfileSource::new(Profile {
            did: Did::new(did.to_string()),
            handle: "artist.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: backend.profile_cache(),
        database,
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// [`spawn_app_on`] over a fresh backend and its plain mem database.
async fn spawn_app(did: &str) -> (String, MemBackend) {
    let backend = MemBackend::new();
    let base = spawn_app_on(did, &backend, backend.database()).await;
    (base, backend)
}

/// A cookie-keeping client that does not auto-follow redirects.
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds")
}

/// Drives the two-step sign-in so the client's cookie jar carries a live session
/// for the app's configured DID.
async fn sign_in(client: &reqwest::Client, base: &str) {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=artist.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(res.status(), 303, "signin should redirect to the PDS");
    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303, "callback should redirect on success");
}

/// Signs in and creates one commission through the API, returning its id.
async fn sign_in_and_create(
    client: &reqwest::Client,
    base: &str,
    backend: &MemBackend,
) -> CommissionId {
    sign_in(client, base).await;
    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "A ref sheet" }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201);
    backend
        .all_commissions()
        .await
        .expect("list commissions")
        .pop()
        .expect("one commission persisted")
        .id
}

/// Seeds a commission owned by an off-session User directly onto the backend.
async fn seed_foreign_commission(backend: &MemBackend, title: &str) -> CommissionId {
    let commission = Commission::create(
        title.parse::<CommissionTitle>().expect("valid title"),
        UserId::new(uuid::Uuid::now_v7()),
        Utc::now(),
        None,
    );
    let id = commission.id;
    backend.create_commission(&commission).await.expect("seed");
    id
}

// AC1 — the owner deletes their fact-free commission: 204, the row is gone
// entirely, and the changelog (its only child table at this stack) cascades away.
#[tokio::test]
async fn owner_deletes_a_fact_free_commission_entirely() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    let id = sign_in_and_create(&client, &base, &backend).await;
    assert!(
        !backend
            .changelog_entries(id)
            .await
            .expect("entries")
            .is_empty(),
        "creation wrote the genesis changelog entry",
    );

    let res = client
        .delete(format!("{base}/commissions/{}", *id))
        .send()
        .await
        .expect("DELETE /commissions/{id}");
    assert_eq!(res.status(), 204, "the delete answers 204 No Content");

    assert!(
        backend.find_commission(id).await.expect("find").is_none(),
        "the commission row is gone",
    );
    assert!(
        backend
            .changelog_entries(id)
            .await
            .expect("entries")
            .is_empty(),
        "the changelog cascaded away with the commission",
    );
}

// AC2 (the closed door) — a signed-in caller who is not a participant of the
// commission gets the uniform 404, never a 403, and deletes nothing.
#[tokio::test]
async fn a_non_participant_gets_the_uniform_404_and_deletes_nothing() {
    let (base, backend) = spawn_app("did:plc:stranger").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = seed_foreign_commission(&backend, "Not yours").await;

    let res = client
        .delete(format!("{base}/commissions/{}", *id))
        .send()
        .await
        .expect("DELETE /commissions/{id}");
    common::assert_problem(res, 404, "commission_not_found").await;

    assert!(
        backend.find_commission(id).await.expect("find").is_some(),
        "the hidden commission survives",
    );
}

// AC2 — a truly absent commission answers the SAME uniform 404 (no existence
// oracle: absent and hidden are indistinguishable).
#[tokio::test]
async fn an_absent_commission_answers_the_same_uniform_404() {
    let (base, _backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;

    let res = client
        .delete(format!("{base}/commissions/{}", uuid::Uuid::now_v7()))
        .send()
        .await
        .expect("DELETE /commissions/{id}");
    common::assert_problem(res, 404, "commission_not_found").await;
}

// The floor — an anonymous caller cannot delete: 401, nothing deleted.
#[tokio::test]
async fn anonymous_cannot_delete_a_commission() {
    let (base, backend) = spawn_app("did:plc:nobody").await;
    let id = seed_foreign_commission(&backend, "Still here").await;

    let res = client()
        .delete(format!("{base}/commissions/{}", *id))
        .send()
        .await
        .expect("DELETE /commissions/{id}");
    common::assert_problem(res, 401, "not_authenticated").await;

    assert!(
        backend.find_commission(id).await.expect("find").is_some(),
        "an unauthenticated delete removes nothing",
    );
}

/// The fact-bearing seam double (AC3): wraps the mem [`Database`] so the vended
/// unit of work answers `commission_has_facts` with `true` — every other call
/// passes through untouched. This stages the state no production code can mint
/// yet, at exactly the port the handler's gate consults.
struct FactBearingDatabase(Arc<dyn Database>);

#[async_trait]
impl Database for FactBearingDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        Ok(Box::new(FactBearingUow(self.0.begin().await?)))
    }
}

/// A pass-through [`UnitOfWork`] whose commissions view claims facts exist.
struct FactBearingUow(Box<dyn UnitOfWork>);

#[async_trait]
impl UnitOfWork for FactBearingUow {
    fn accounts(&mut self) -> Box<dyn AccountWrites + '_> {
        self.0.accounts()
    }

    fn commissions(&mut self) -> Box<dyn CommissionWrites + '_> {
        Box::new(FactBearingCommissions(self.0.commissions()))
    }

    fn changelog(&mut self) -> Box<dyn ChangelogWrites + '_> {
        self.0.changelog()
    }

    fn users(&mut self) -> Box<dyn UserWrites + '_> {
        self.0.users()
    }

    fn actor_identities(&mut self) -> Box<dyn ActorIdentityWrites + '_> {
        self.0.actor_identities()
    }

    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        self.0.commit().await
    }

    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        self.0.rollback().await
    }
}

/// The commissions view of [`FactBearingUow`]: `commission_has_facts` is `true`,
/// everything else delegates — so a gate that wrongly proceeded to `delete`
/// would really delete, and the test would catch it by the row's disappearance.
struct FactBearingCommissions<'a>(Box<dyn CommissionWrites + 'a>);

#[async_trait]
impl CommissionWrites for FactBearingCommissions<'_> {
    async fn create(&mut self, commission: &Commission) -> anyhow::Result<()> {
        self.0.create(commission).await
    }

    async fn create_seat_invitation(
        &mut self,
        invitation: &domain::elements::commission::SeatInvitation,
    ) -> anyhow::Result<()> {
        self.0.create_seat_invitation(invitation).await
    }

    async fn revoke_seat_invitation(
        &mut self,
        id: domain::elements::commission::SeatInvitationId,
    ) -> anyhow::Result<()> {
        self.0.revoke_seat_invitation(id).await
    }

    async fn commission_has_facts(&mut self, _id: CommissionId) -> anyhow::Result<bool> {
        Ok(true)
    }

    async fn set_linked_channel(
        &mut self,
        id: CommissionId,
        channel: Option<&ChannelPointer>,
    ) -> anyhow::Result<bool> {
        self.0.set_linked_channel(id, channel).await
    }

    async fn delete(&mut self, id: CommissionId) -> anyhow::Result<()> {
        self.0.delete(id).await
    }

    async fn set_archived(
        &mut self,
        id: CommissionId,
        archived_at: Option<domain::datetime::DateTimeUtc>,
    ) -> anyhow::Result<bool> {
        self.0.set_archived(id, archived_at).await
    }

    async fn add_surface(&mut self, surface: &NewSurface) -> anyhow::Result<()> {
        self.0.add_surface(surface).await
    }

    async fn add_component(&mut self, component: &NewComponent) -> anyhow::Result<()> {
        self.0.add_component(component).await
    }

    async fn remove_node(&mut self, commission: CommissionId, node: NodeId) -> anyhow::Result<()> {
        self.0.remove_node(commission, node).await
    }

    async fn set_maturity(&mut self, id: CommissionId, maturity: Maturity) -> anyhow::Result<()> {
        self.0.set_maturity(id, maturity).await
    }

    async fn add_file(
        &mut self,
        file: &domain::elements::commission::CommissionFile,
    ) -> anyhow::Result<()> {
        self.0.add_file(file).await
    }

    async fn place(
        &mut self,
        commission: CommissionId,
        account: AccountId,
        actor: UserId,
        at: domain::datetime::DateTimeUtc,
    ) -> anyhow::Result<()> {
        self.0.place(commission, account, actor, at).await
    }

    async fn grant_view(
        &mut self,
        commission: CommissionId,
        account: AccountId,
        level: GrantLevel,
    ) -> anyhow::Result<()> {
        self.0.grant_view(commission, account, level).await
    }

    async fn revoke_view(
        &mut self,
        commission: CommissionId,
        account: AccountId,
    ) -> anyhow::Result<bool> {
        self.0.revoke_view(commission, account).await
    }

    async fn set_direction_status(
        &mut self,
        id: CommissionId,
        status: Option<domain::elements::commission::DirectionStatus>,
    ) -> anyhow::Result<bool> {
        self.0.set_direction_status(id, status).await
    }

    async fn set_deadline(
        &mut self,
        id: CommissionId,
        deadline: Option<domain::datetime::DateTimeUtc>,
    ) -> anyhow::Result<bool> {
        self.0.set_deadline(id, deadline).await
    }

    async fn set_deadline_status(
        &mut self,
        id: CommissionId,
        status: Option<domain::elements::commission::DeadlineStatus>,
    ) -> anyhow::Result<bool> {
        self.0.set_deadline_status(id, status).await
    }

    async fn lapsed_deadlines(
        &mut self,
        now: domain::datetime::DateTimeUtc,
    ) -> anyhow::Result<Vec<domain::elements::commission::LapsedDeadline>> {
        self.0.lapsed_deadlines(now).await
    }

    async fn declare_slots(
        &mut self,
        slots: &[domain::elements::commission::NewSlot],
    ) -> anyhow::Result<()> {
        self.0.declare_slots(slots).await
    }

    async fn declare_seat(
        &mut self,
        seat: &domain::elements::commission::NewSeat,
    ) -> anyhow::Result<()> {
        self.0.declare_seat(seat).await
    }
}

// AC3 — deleting a fact-bearing commission is refused with the 409
// `commission_has_facts` problem, its detail points the caller at Archive, and
// the commission (with its changelog) survives untouched.
#[tokio::test]
async fn a_fact_bearing_commission_is_refused_and_pointed_at_archive() {
    let backend = MemBackend::new();
    let database: Arc<dyn Database> = Arc::new(FactBearingDatabase(backend.database()));
    let base = spawn_app_on("did:plc:artist", &backend, database).await;
    let client = client();
    let id = sign_in_and_create(&client, &base, &backend).await;

    let res = client
        .delete(format!("{base}/commissions/{}", *id))
        .send()
        .await
        .expect("DELETE /commissions/{id}");
    assert_eq!(res.status().as_u16(), 409, "a fact-bearing delete is a 409");
    assert_eq!(
        res.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .expect("content-type is set"),
        "application/problem+json",
    );
    let body: serde_json::Value = res.json().await.expect("problem body");
    assert_eq!(body["code"], "commission_has_facts");
    assert_eq!(body["type"], "urn:zurfur:error:commission-has-facts");
    assert_eq!(body["status"], 409);
    assert!(
        body["detail"]
            .as_str()
            .is_some_and(|d| d.to_lowercase().contains("archive")),
        "the detail points the caller at Archive, got {:?}",
        body["detail"],
    );

    assert!(
        backend.find_commission(id).await.expect("find").is_some(),
        "a refused delete removes nothing",
    );
    assert!(
        !backend
            .changelog_entries(id)
            .await
            .expect("entries")
            .is_empty(),
        "the changelog survives with its commission",
    );
}
