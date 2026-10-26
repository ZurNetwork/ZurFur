//! The owner hard-deletes a fact-free commission, end to end over HTTP.
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
//!   shared `require_owner` seam is unreachable until non-owner
//!   participants can be seated on a commission.)
//! - **AC3** — deleting a fact-bearing commission is rejected with the `409`
//!   `commission_has_facts` problem whose detail points at Archive, and nothing is
//!   deleted. No fact-minter exists yet, so the fact-bearing state is staged with
//!   a test double at the [`Database`] port seam: it wraps the mem unit of work
//!   and answers `commission_has_facts` with `true`, proving the handler's gate
//!   consults the predicate **inside the unit of work that would delete** and
//!   refuses.
//! - A commission past Draft (Active, or Disputed) is refused with the same `409`
//!   and nothing goes; one disputed from Draft and cleared back is in Draft again
//!   and deletes. The step is read under lock in the deleting unit: the same
//!   double stages a commission the pool reads in Draft and the locked read finds
//!   Active, and the delete is refused.
//!
//! Same in-process fakes as the other api e2e suites, except the last test, which
//! runs the API on PostgreSQL and requires a container runtime socket.

use std::sync::Arc;

use adapter_mem::{CommissionSeed, MemBackend};
use api::AppState;
use async_trait::async_trait;
use domain::datetime::DateTimeUtc;
use domain::elements::{
    commission::{
        AsOwner, AsParticipant, ChannelPointer, Commission, CommissionBirth, CommissionFile,
        CommissionId, CommissionMarkup, CommissionTitle, DeadlineChange, DelayedChange,
        DirectionChange, Dispute, DisputeClear, ElementId, GrantLevel, LateKinds, LifecycleMove,
        LifecycleStep, LockedNode, LockedStanding, MaturitySet, NewCommission, NewElement, NewSeat,
        NewSlot, SeatInvitation, SeatInvitationId, TabId, element::TabRow,
    },
    did::Did,
    profile::Profile,
    user::UserId,
};
use domain::ports::{
    AccountRepo, ActorIdentityWrites, ChangelogWrites, ColumnWrites, CommissionReads,
    CommissionRepo, CommissionWrites, Creator, Database, New, UnitOfWork, UserWrites,
    WorkflowWrites, WriteCapability,
};
use serde_json::{Value, json};
use test_support::{
    den::get,
    http::{client, serve, sign_in},
    pg_app,
};

mod common;

/// Boots the app around a caller-supplied backend + database pair, so a test can
/// interpose at the unit-of-work seam while reads keep hitting the same backend.
/// `did` is the identity `sign_in` will authenticate as.
async fn spawn_app_on(did: &str, backend: &MemBackend, database: Arc<dyn Database>) -> String {
    // A throwaway runtime supplies config/pool/auth/profile_source/did_minter;
    // the store fields below are overridden onto the CALLER's backend (and a
    // possibly-interposed `database`) so the seam double in this file
    // (`StagedDatabase`) can wrap the same backend it reads through.
    let backend = backend.clone();
    let served = serve(
        test_support::runtime::mem(&Did::from(did.to_string())).profile(Profile::new(
            Did::from(did.to_string()),
            "artist.bsky.social",
        )),
        move |rt| {
            api::app(AppState {
                files: backend.file_store(),
                users: backend.user_store(),
                profile_cache: backend.profile_cache(),
                database,
                accounts: backend.account_store(),
                commissions: backend.commission_store(),
                changelog: backend.changelog_store(),
                ..rt
            })
        },
    )
    .await;
    served.base_url
}

/// [`spawn_app_on`] over a fresh backend and its plain mem database.
async fn spawn_app(did: &str) -> (String, MemBackend) {
    let backend = MemBackend::new();
    let base = spawn_app_on(did, &backend, backend.database()).await;
    (base, backend)
}

/// Signs in and creates one commission through the API, returning its id.
async fn sign_in_and_create(
    client: &reqwest::Client,
    base: &str,
    backend: &MemBackend,
) -> CommissionId {
    sign_in(client, base, "artist.bsky.social").await;
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
    let owner = backend
        .provision(&Did::from("did:plc:offsession-owner".to_string()))
        .await
        .expect("provision the off-session owner");
    let title = title.parse::<CommissionTitle>().expect("valid title");
    let seed = CommissionSeed::private(title, owner.id);
    backend.seed_bound_commission(&seed).await.expect("seed");
    seed.id
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
        .delete(format!("{base}/commissions/{}", id))
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
    sign_in(&client, &base, "artist.bsky.social").await;
    let id = seed_foreign_commission(&backend, "Not yours").await;

    let res = client
        .delete(format!("{base}/commissions/{}", id))
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
    sign_in(&client, &base, "artist.bsky.social").await;

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
        .delete(format!("{base}/commissions/{}", id))
        .send()
        .await
        .expect("DELETE /commissions/{id}");
    common::assert_problem(res, 401, "not_authenticated").await;

    assert!(
        backend.find_commission(id).await.expect("find").is_some(),
        "an unauthenticated delete removes nothing",
    );
}

/// What [`StagedDatabase`]'s units claim about every commission, beyond what the store holds.
#[derive(Debug, Clone, Copy)]
enum Staged {
    /// `commission_has_facts` answers `true` (AC3): the state no production code can mint yet.
    Facts,
    /// The locked read finds the commission Active, as if a Participant's move committed
    /// between the pool read and the unit; the pool read still sees it in Draft.
    MovedPastDraft,
}

/// The seam double: wraps the mem [`Database`] so the vended unit of work claims its
/// [`Staged`] state, at exactly the port the handler's gate consults; every other call
/// passes through untouched.
struct StagedDatabase {
    inner: Arc<dyn Database>,
    staged: Staged,
}

#[async_trait]
impl Database for StagedDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        let unit = StagedUow {
            inner: self.inner.begin().await?,
            staged: self.staged,
        };
        Ok(Box::new(unit))
    }
}

/// A pass-through [`UnitOfWork`] whose commissions view claims the [`Staged`] state.
struct StagedUow {
    inner: Box<dyn UnitOfWork>,
    staged: Staged,
}

#[async_trait]
impl UnitOfWork for StagedUow {
    fn accounts(&mut self) -> Box<dyn AccountRepo + '_> {
        self.inner.accounts()
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "a unit-of-work double wraps the commission repo of the unit it wraps"
    )]
    fn commissions(&mut self) -> Box<dyn CommissionRepo + '_> {
        let staged = self.staged;
        let commissions = StagedCommissions {
            inner: self.inner.commissions(),
            staged,
        };
        Box::new(commissions)
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "a unit-of-work double hands out the changelog of the unit it wraps"
    )]
    fn changelog(&mut self) -> Box<dyn ChangelogWrites + '_> {
        self.inner.changelog()
    }

    fn users(&mut self) -> Box<dyn UserWrites + '_> {
        self.inner.users()
    }

    fn characters(&mut self) -> Box<dyn domain::ports::character::CharacterWrites + '_> {
        self.inner.characters()
    }

    fn bug_reports(&mut self) -> Box<dyn domain::ports::BugReportWrites + '_> {
        self.inner.bug_reports()
    }

    #[expect(
        clippy::disallowed_methods,
        reason = "a unit-of-work double hands out the node repo of the unit it wraps"
    )]
    fn nodes(&mut self) -> Box<dyn domain::ports::NodeRepo + '_> {
        self.inner.nodes()
    }

    fn actor_identities(&mut self) -> Box<dyn ActorIdentityWrites + '_> {
        self.inner.actor_identities()
    }

    fn workflows(&mut self) -> Box<dyn WorkflowWrites + '_> {
        self.inner.workflows()
    }

    fn columns(&mut self) -> Box<dyn ColumnWrites + '_> {
        self.inner.columns()
    }

    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        self.inner.commit().await
    }

    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        self.inner.rollback().await
    }
}

/// The commissions view of [`StagedUow`]: its [`Staged`] call answers the staged state,
/// everything else delegates — so a gate that wrongly proceeded to `delete`
/// would really delete, and the test would catch it by the row's disappearance.
/// [`CommissionRepo`] is a blanket marker over reads + writes, so both halves
/// are implemented here.
struct StagedCommissions<'a> {
    inner: Box<dyn CommissionRepo + 'a>,
    staged: Staged,
}

#[async_trait]
impl CommissionReads for StagedCommissions<'_> {
    async fn find(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        self.inner.find(id).await
    }

    async fn find_for_update(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>> {
        let mut locked = self.inner.find_for_update(id).await?;
        if let (Staged::MovedPastDraft, Some(commission)) = (self.staged, locked.as_mut()) {
            commission.lifecycle_step = LifecycleStep::Active;
        }
        Ok(locked)
    }

    async fn standing_for_update(
        &mut self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<Option<LockedStanding>> {
        self.inner.standing_for_update(commission, user).await
    }

    async fn lock_for_system(
        &mut self,
        commission: &CommissionId,
    ) -> anyhow::Result<Option<LockedNode>> {
        self.inner.lock_for_system(commission).await
    }

    async fn late_owed(
        &mut self,
        commission: &CommissionId,
        late_kinds: &LateKinds,
    ) -> anyhow::Result<bool> {
        self.inner.late_owed(commission, late_kinds).await
    }

    async fn is_participant(
        &mut self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<bool> {
        self.inner.is_participant(commission, user).await
    }

    async fn tab_for_update(
        &mut self,
        commission: &CommissionId,
        tab: &TabId,
    ) -> anyhow::Result<Option<TabRow>> {
        self.inner.tab_for_update(commission, tab).await
    }
}

#[async_trait]
impl CommissionWrites for StagedCommissions<'_> {
    async fn commission_has_facts(&mut self, id: &CommissionId) -> anyhow::Result<bool> {
        match self.staged {
            Staged::Facts => Ok(true),
            Staged::MovedPastDraft => self.inner.commission_has_facts(id).await,
        }
    }

    async fn create(
        &mut self,
        capability: &WriteCapability<New<CommissionId>, Creator>,
        birth: &CommissionBirth,
        commission: &NewCommission,
    ) -> anyhow::Result<()> {
        self.inner.create(capability, birth, commission).await
    }

    async fn set_lifecycle(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &LifecycleMove,
    ) -> anyhow::Result<bool> {
        self.inner.set_lifecycle(capability, change).await
    }

    async fn set_dispute(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        dispute: &Dispute,
    ) -> anyhow::Result<bool> {
        self.inner.set_dispute(capability, dispute).await
    }

    async fn clear_dispute(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        clear: &DisputeClear,
    ) -> anyhow::Result<bool> {
        self.inner.clear_dispute(capability, clear).await
    }

    async fn declare_seat(&mut self, seat: &NewSeat) -> anyhow::Result<()> {
        self.inner.declare_seat(seat).await
    }

    async fn create_seat_invitation(
        &mut self,
        invitation: &SeatInvitation,
    ) -> anyhow::Result<SeatInvitation> {
        self.inner.create_seat_invitation(invitation).await
    }

    async fn revoke_seat_invitation(&mut self, id: &SeatInvitationId) -> anyhow::Result<()> {
        self.inner.revoke_seat_invitation(id).await
    }

    async fn add_element(&mut self, element: &NewElement) -> anyhow::Result<()> {
        self.inner.add_element(element).await
    }

    async fn remove_element(
        &mut self,
        commission: &CommissionId,
        element: &ElementId,
    ) -> anyhow::Result<()> {
        self.inner.remove_element(commission, element).await
    }

    async fn add_file(&mut self, file: &CommissionFile) -> anyhow::Result<()> {
        self.inner.add_file(file).await
    }

    async fn add_markup(&mut self, markup: &CommissionMarkup) -> anyhow::Result<()> {
        self.inner.add_markup(markup).await
    }

    async fn declare_slots(&mut self, slots: &[NewSlot]) -> anyhow::Result<()> {
        self.inner.declare_slots(slots).await
    }

    async fn delete(&mut self, id: &CommissionId) -> anyhow::Result<()> {
        self.inner.delete(id).await
    }

    async fn set_archived(
        &mut self,
        id: &CommissionId,
        archived_at: Option<DateTimeUtc>,
    ) -> anyhow::Result<bool> {
        self.inner.set_archived(id, archived_at).await
    }

    async fn set_maturity(
        &mut self,
        capability: &WriteCapability<CommissionId, AsOwner>,
        change: &MaturitySet,
    ) -> anyhow::Result<bool> {
        self.inner.set_maturity(capability, change).await
    }

    async fn set_linked_channel(
        &mut self,
        id: &CommissionId,
        channel: Option<&ChannelPointer>,
    ) -> anyhow::Result<bool> {
        self.inner.set_linked_channel(id, channel).await
    }

    async fn grant_view(
        &mut self,
        commission: &CommissionId,
        to_user: &UserId,
        level: GrantLevel,
    ) -> anyhow::Result<()> {
        self.inner.grant_view(commission, to_user, level).await
    }

    async fn revoke_view(
        &mut self,
        commission: &CommissionId,
        to_user: &UserId,
    ) -> anyhow::Result<bool> {
        self.inner.revoke_view(commission, to_user).await
    }

    async fn set_direction_status(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &DirectionChange,
    ) -> anyhow::Result<bool> {
        self.inner.set_direction_status(capability, change).await
    }

    async fn set_deadline(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &DeadlineChange,
    ) -> anyhow::Result<bool> {
        self.inner.set_deadline(capability, change).await
    }

    async fn set_deadline_status(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &DelayedChange,
    ) -> anyhow::Result<bool> {
        self.inner.set_deadline_status(capability, change).await
    }
}

// AC3 — deleting a fact-bearing commission is refused with the 409
// `commission_has_facts` problem, its detail points the caller at Archive, and
// the commission (with its changelog) survives untouched.
#[tokio::test]
async fn a_fact_bearing_commission_is_refused_and_pointed_at_archive() {
    let backend = MemBackend::new();
    let staged = StagedDatabase {
        inner: backend.database(),
        staged: Staged::Facts,
    };
    let database: Arc<dyn Database> = Arc::new(staged);
    let base = spawn_app_on("did:plc:artist", &backend, database).await;
    let client = client();
    let id = sign_in_and_create(&client, &base, &backend).await;

    let res = client
        .delete(format!("{base}/commissions/{}", id))
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

// The step is decided on the row read under lock in the deleting unit, never on the pool read:
// a commission the pool still reads in Draft, which the locked read finds Active (a move
// committed in between), is refused with the 409 and nothing goes.
#[tokio::test]
async fn the_step_is_checked_on_the_locked_read_not_the_pool_read() {
    let backend = MemBackend::new();
    let staged = StagedDatabase {
        inner: backend.database(),
        staged: Staged::MovedPastDraft,
    };
    let database: Arc<dyn Database> = Arc::new(staged);
    let base = spawn_app_on("did:plc:artist", &backend, database).await;
    let client = client();
    let id = sign_in_and_create(&client, &base, &backend).await;

    let res = delete_commission(&client, &base, &id.to_string()).await;

    common::assert_problem(res, 409, "commission_has_facts").await;
    assert!(
        backend.find_commission(id).await.expect("find").is_some(),
        "a refused delete removes nothing",
    );
}

/// `DELETE {base}/commissions/{id}` as `client`.
async fn delete_commission(client: &reqwest::Client, base: &str, id: &str) -> reqwest::Response {
    client
        .delete(format!("{base}/commissions/{id}"))
        .send()
        .await
        .expect("DELETE /commissions/{id}")
}

/// `PUT {base}/commissions/{id}/lifecycle` as `client`, moving it to `step`; asserts the `204`.
async fn move_to(client: &reqwest::Client, base: &str, id: &str, step: &str) {
    let body = json!({ "to": step });
    let res = client
        .put(format!("{base}/commissions/{id}/lifecycle"))
        .json(&body)
        .send()
        .await
        .expect("PUT …/lifecycle");
    assert_eq!(res.status(), 204, "the move to {step} answers 204");
}

/// `method {base}/commissions/{id}/dispute` as `client`; asserts the `204`.
async fn dispute(client: &reqwest::Client, base: &str, id: &str, method: reqwest::Method) {
    let res = client
        .request(method.clone(), format!("{base}/commissions/{id}/dispute"))
        .send()
        .await
        .expect("…/dispute");
    assert_eq!(res.status(), 204, "{method} …/dispute answers 204");
}

// The owner's delete of an Active commission is refused with the 409 `commission_has_facts`,
// and nothing goes: the row and every changelog entry stay.
#[tokio::test]
async fn an_active_commission_is_refused_and_nothing_goes() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    let id = sign_in_and_create(&client, &base, &backend).await;
    let id_text = id.to_string();
    move_to(&client, &base, &id_text, "active").await;
    let entries_before = backend.changelog_entries(id).await.expect("entries").len();

    let res = delete_commission(&client, &base, &id_text).await;

    common::assert_problem(res, 409, "commission_has_facts").await;
    assert!(
        backend.find_commission(id).await.expect("find").is_some(),
        "a refused delete removes nothing",
    );
    let entries_after = backend.changelog_entries(id).await.expect("entries").len();
    assert_eq!(entries_after, entries_before, "every entry stays");
}

// A commission disputed from Draft is Disputed, so its delete is refused; cleared back, it is
// in Draft again and deletes: 204, the row gone and its changelog with it.
#[tokio::test]
async fn a_dispute_from_draft_refuses_until_cleared_then_deletes() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    let id = sign_in_and_create(&client, &base, &backend).await;
    let id_text = id.to_string();
    dispute(&client, &base, &id_text, reqwest::Method::PUT).await;

    let while_disputed = delete_commission(&client, &base, &id_text).await;

    common::assert_problem(while_disputed, 409, "commission_has_facts").await;
    assert!(
        backend.find_commission(id).await.expect("find").is_some(),
        "a Disputed commission survives its delete",
    );
    dispute(&client, &base, &id_text, reqwest::Method::DELETE).await;

    let cleared = delete_commission(&client, &base, &id_text).await;

    assert_eq!(
        cleared.status(),
        204,
        "back in Draft, the delete answers 204"
    );
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

/// The kind of each of `commission`'s changelog entries, in order, as `client` reads them.
async fn changelog_kinds(
    client: &reqwest::Client,
    world: &pg_app::DenWorld,
    id: &str,
) -> Vec<String> {
    let path = format!("/commissions/{id}/changelog");
    let res = get(client, world.base_url(), &path).await;
    assert_eq!(res.status(), 200, "the changelog reads");
    let body: Value = res.json().await.expect("a JSON body");
    body.as_array()
        .expect("an array of entries")
        .iter()
        .map(|entry| entry["kind"].as_str().expect("a kind").to_owned())
        .collect()
}

// On PostgreSQL: bob, a Participant, moves alice's "Untitled" to Active and raises a dispute;
// alice's delete answers the 409 `commission_has_facts`, the commission still reads, and its
// changelog keeps all four entries.
#[tokio::test]
async fn on_postgres_a_disputed_active_commission_is_refused_and_keeps_its_entries() {
    let world = pg_app::den_world(api::app).await;
    let base = world.base_url().to_owned();
    let untitled = world.untitled.id.to_string();
    move_to(&world.bob, &base, &untitled, "active").await;
    dispute(&world.bob, &base, &untitled, reqwest::Method::PUT).await;
    let kinds_before = changelog_kinds(&world.alice, &world, &untitled).await;
    let four_kinds = [
        "created",
        "file_added",
        "lifecycle_moved",
        "lifecycle_moved",
    ];
    assert_eq!(kinds_before, four_kinds);

    let res = delete_commission(&world.alice, &base, &untitled).await;

    common::assert_problem(res, 409, "commission_has_facts").await;
    let read = get(&world.alice, &base, &format!("/commissions/{untitled}")).await;
    assert_eq!(read.status(), 200, "the commission still reads");
    let kinds_after = changelog_kinds(&world.alice, &world, &untitled).await;
    assert_eq!(kinds_after, four_kinds, "every entry stays");
}
