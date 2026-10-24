use axum::Router;
use chrono::Utc;
use composition::Runtime;
use domain::elements::{
    account::AccountId,
    commission::{CommissionId, ElementId, FileKey, LifecycleStep, SKELETON},
    did::Did,
    level::Level,
    profile::Profile,
    user::UserId,
    workflow::{ColumnId, WorkflowId},
};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{PgServed, StatementLog, serve, serve_recording};

mod identity;

pub use identity::{absent_account, stranger};

/// The stranger's DID: a user who signs in and takes part in nothing.
const STRANGER_DID: &str = "did:plc:s5trstrangerfixtureaaaaa";

/// The level token no parser reads, for the node a stored token can't be parsed from.
const UNPARSABLE_LEVEL: &str = "not-a-visibility";

/// The lifecycle token no parser reads, for the row only the whole-commission read parses.
const UNPARSABLE_LIFECYCLE: &str = "not-a-lifecycle";

/// A name stored before today's name rule, which the rule now refuses: its right-to-left
/// override shows it as `photoexe.jpg`.
pub const LEGACY_NAME: &str = "photo\u{202E}gpj.exe";

/// The name the Den shows, and the download serves, for a stored name the rule refuses.
pub const UNNAMED: &str = "Unnamed";

/// A commission id no seed writes: a UUIDv7 from 2025, before any row's.
const ABSENT_COMMISSION: Uuid = Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0a01);
/// A file key no seed writes.
const ABSENT_FILE: Uuid = Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0a02);
/// A slot id no seed writes.
const ABSENT_SLOT: Uuid = Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0a03);
/// A Workflow id no seed writes.
const ABSENT_WORKFLOW: Uuid = Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0a04);

/// The profile cache's state a read starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::VariantArray)]
pub enum ProfileCacheState {
    /// Every signed-in user's profile cached afresh: each root is named from the cache.
    #[strum(serialize = "warm cache")]
    Warm,
    /// The cache empty: each root's name is looked up, and the lookup fills the cache.
    #[strum(serialize = "cold cache")]
    Cold,
}

/// A commission the world seeds: its id, the one file uploaded to it and its one slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeededCommission {
    /// The commission's id.
    pub id: CommissionId,
    /// The key of the one file uploaded to it.
    pub file: FileKey,
    /// The id of its one slot's carrying element.
    pub slot: ElementId,
}

/// The den world on PostgreSQL, everyone signed in: alice, who owns the Account "Supreme
/// Arts" (its Workflow "Queue") and the commissions "Untitled" and the archived "Old
/// sketch"; bob, a Participant of "Untitled", who owns "Night Shift" (its Workflow
/// "Backlog"); and a stranger, part of nothing.
pub struct DenWorld {
    /// The served app, its live runtime and its database.
    pub served: PgServed,
    /// A client signed in as alice.
    pub alice: reqwest::Client,
    /// A client signed in as bob.
    pub bob: reqwest::Client,
    /// A client signed in as the stranger.
    pub stranger: reqwest::Client,
    /// alice's Account.
    pub supreme_arts: AccountId,
    /// bob's Account.
    pub night_shift: AccountId,
    /// The Workflow on "Supreme Arts"'s board.
    pub queue: WorkflowId,
    /// The Workflow on "Night Shift"'s board.
    pub backlog: WorkflowId,
    /// alice's live commission, bob a Participant.
    pub untitled: SeededCommission,
    /// alice's archived commission, hers alone.
    pub old_sketch: SeededCommission,
}

/// Serves `app` on a fresh PostgreSQL database and seeds the [`DenWorld`] through the app's
/// routes and use cases. Two writes no route makes yet are test-only: bob's participation in
/// "Untitled", and any level other than Private ([`DenWorld::set_visibility`]).
pub async fn den_world(app: impl FnOnce(Runtime) -> Router) -> DenWorld {
    let served = serve(app).await;
    seed(served).await
}

/// As [`den_world`], with every statement the app executes on PostgreSQL logged into
/// `statements`, the seeding's included.
pub async fn den_world_recording(
    app: impl FnOnce(Runtime) -> Router,
    statements: &StatementLog,
) -> DenWorld {
    let served = serve_recording(app, statements).await;
    seed(served).await
}

/// Seeds the [`DenWorld`] on `served`.
#[expect(
    clippy::let_and_return,
    reason = "the style rulebook names a multi-line construction before returning it"
)]
async fn seed(served: PgServed) -> DenWorld {
    let alice = sign_in(&served, alice_profile()).await;
    let bob = sign_in(&served, bob_profile()).await;
    let stranger = sign_in(&served, stranger_profile()).await;

    let supreme_arts = found_account(&served, &alice, "Supreme Arts", "supreme-arts").await;
    let night_shift = found_account(&served, &bob, "Night Shift", "night-shift").await;
    let queue = create_workflow(
        &served,
        adapter_mem::fixture::alice(),
        &supreme_arts,
        "Queue",
    )
    .await;
    let backlog = create_workflow(
        &served,
        adapter_mem::fixture::bob(),
        &night_shift,
        "Backlog",
    )
    .await;

    let untitled = seed_commission(&served, &alice, "Untitled", "ref-abco.png", "Abco").await;
    let old_sketch =
        seed_commission(&served, &alice, "Old sketch", "old-sketch.png", "Ember").await;
    let archive_path = format!("/commissions/{}/archive", old_sketch.id);
    let no_body = json!({});
    let archived = post_json(&alice, &served, &archive_path, &no_body).await;
    assert!(
        archived.status().is_success(),
        "archive: {}",
        archived.status()
    );

    let bob_did = adapter_mem::fixture::bob().to_string();
    adapter_pg::queries::commission::add_participant(
        &served.runtime.pool,
        Uuid::from(untitled.id),
        &bob_did,
        Utc::now(),
    )
    .await
    .expect("bob joins Untitled as a Participant");

    let world = DenWorld {
        served,
        alice,
        bob,
        stranger,
        supreme_arts,
        night_shift,
        queue,
        backlog,
        untitled,
        old_sketch,
    };
    world
}

impl DenWorld {
    /// The served app's base URL.
    pub fn base_url(&self) -> &str {
        &self.served.base_url
    }

    /// Sets the level of `commission`'s node, a test-only update: no route sets it yet.
    pub async fn set_visibility(&self, commission: CommissionId, visibility: Level) {
        self.store_level(commission, &visibility.to_string()).await;
    }

    /// Stores a level token no parser reads on `commission`'s node, a test-only update. The
    /// node table refuses one, so this drops its `node_level_known` check in this world's own
    /// database first.
    pub async fn store_unparsable_level(&self, commission: CommissionId) {
        sqlx::query("ALTER TABLE node DROP CONSTRAINT IF EXISTS node_level_known")
            .execute(&self.served.runtime.pool)
            .await
            .expect("drop node_level_known in the world's database");
        self.store_level(commission, UNPARSABLE_LEVEL).await;
    }

    /// Puts `commission` at the lifecycle `step`, a test-only update: no route moves it into
    /// Disputed.
    pub async fn set_lifecycle_step(&self, commission: CommissionId, step: LifecycleStep) {
        let updated = sqlx::query("UPDATE commission SET lifecycle = $1 WHERE id = $2")
            .bind(step.to_string())
            .bind(Uuid::from(commission))
            .execute(&self.served.runtime.pool)
            .await
            .expect("update the commission's lifecycle");
        assert_eq!(updated.rows_affected(), 1, "one commission row is updated");
    }

    /// Stores a lifecycle token no parser reads on `commission`'s row, a test-only update.
    pub async fn store_unparsable_lifecycle(&self, commission: CommissionId) {
        let updated = sqlx::query("UPDATE commission SET lifecycle = $1 WHERE id = $2")
            .bind(UNPARSABLE_LIFECYCLE)
            .bind(Uuid::from(commission))
            .execute(&self.served.runtime.pool)
            .await
            .expect("update the commission's lifecycle");
        assert_eq!(updated.rows_affected(), 1, "one commission row is updated");
    }

    /// Stores [`LEGACY_NAME`] as `commission`'s name, as a commission named before today's
    /// rule holds it: on its node and in its created entry's payload. Test-only updates in one
    /// transaction, which alone switches the changelog's append-only trigger off.
    pub async fn store_legacy_title(&self, commission: CommissionId) {
        let mut tx = self
            .served
            .runtime
            .pool
            .begin()
            .await
            .expect("begin a transaction");
        let titled = sqlx::query(
            "UPDATE node SET name = $1 FROM commission c WHERE c.id = $2 AND node.id = c.node_id",
        )
        .bind(LEGACY_NAME)
        .bind(Uuid::from(commission))
        .execute(&mut *tx)
        .await
        .expect("update the commission node's name");
        sqlx::query(
            "ALTER TABLE commission_changelog DISABLE TRIGGER commission_changelog_append_only",
        )
        .execute(&mut *tx)
        .await
        .expect("switch the append-only trigger off");
        let recorded = sqlx::query(
            "UPDATE commission_changelog \
             SET payload = jsonb_set(payload, '{title}', to_jsonb($1::text)) \
             WHERE commission_id = $2 AND kind = 'created'",
        )
        .bind(LEGACY_NAME)
        .bind(Uuid::from(commission))
        .execute(&mut *tx)
        .await
        .expect("update the created entry's payload");
        sqlx::query(
            "ALTER TABLE commission_changelog ENABLE TRIGGER commission_changelog_append_only",
        )
        .execute(&mut *tx)
        .await
        .expect("switch the append-only trigger back on");
        tx.commit().await.expect("commit the legacy title");
        assert_eq!(titled.rows_affected(), 1, "one node is updated");
        assert_eq!(recorded.rows_affected(), 1, "one created entry is updated");
    }

    /// Stores [`LEGACY_NAME`] as `file`'s name, as a file uploaded before today's rule holds
    /// it. A test-only update.
    pub async fn store_legacy_filename(&self, file: FileKey) {
        let renamed = sqlx::query("UPDATE file_blob SET filename = $1 WHERE key = $2")
            .bind(LEGACY_NAME)
            .bind(Uuid::from(file))
            .execute(&self.served.runtime.pool)
            .await
            .expect("update the file's name");
        assert_eq!(renamed.rows_affected(), 1, "one blob is renamed");
    }

    /// Makes bob a Participant of the archived "Old sketch" too, a test-only insert as for
    /// "Untitled". Opt-in: the world's other pairs use "Old sketch" as a commission bob isn't in.
    pub async fn add_bob_to_old_sketch(&self) {
        let bob_did = adapter_mem::fixture::bob().to_string();
        adapter_pg::queries::commission::add_participant(
            &self.served.runtime.pool,
            Uuid::from(self.old_sketch.id),
            &bob_did,
            Utc::now(),
        )
        .await
        .expect("bob joins Old sketch as a Participant");
    }

    /// `"archived"` for "Old sketch", `"live"` for any other seeded commission, for labels.
    pub fn archived_or_live(&self, commission: &SeededCommission) -> &'static str {
        if *commission == self.old_sketch {
            "archived"
        } else {
            "live"
        }
    }

    /// Puts the profile cache in `state` for alice, bob and the stranger alike.
    pub async fn set_profile_cache(&self, state: ProfileCacheState) {
        match state {
            ProfileCacheState::Warm => self.warm_profile_cache().await,
            ProfileCacheState::Cold => self.empty_profile_cache().await,
        }
    }

    /// Caches alice's, bob's and the stranger's profiles afresh, as their PDS answers them.
    async fn warm_profile_cache(&self) {
        let runtime = &self.served.runtime;
        for profile in [alice_profile(), bob_profile(), stranger_profile()] {
            runtime
                .profile_cache
                .put(&profile)
                .await
                .expect("the profile is cached");
        }
    }

    /// Empties the profile cache.
    async fn empty_profile_cache(&self) {
        sqlx::query("DELETE FROM profile_cache")
            .execute(&self.served.runtime.pool)
            .await
            .expect("empty the profile cache");
    }

    /// Adds the column `name` to "Night Shift"'s Workflow "Backlog" as bob, through the add
    /// use case, and answers its id.
    pub async fn add_backlog_column(&self, name: &str) -> ColumnId {
        let command = application::account::workflow::column::add::Command {
            actor_id: adapter_mem::fixture::bob(),
            workflow_id: self.backlog,
            column_name: name.parse().expect("a valid column name"),
            position: 0,
        };
        let app = self.served.runtime.app();
        let accounts = app.accounts();
        let added = accounts
            .workflows()
            .columns()
            .add(command)
            .await
            .expect("the column is added");
        added
            .columns
            .into_iter()
            .find(|column| column.name.as_ref() == name)
            .expect("the added column is on the board")
            .id
    }

    /// Writes `token` as the level of the node `commission` points at and as the commission's
    /// own visibility column, keeping the two equal: no trigger copies a level.
    async fn store_level(&self, commission: CommissionId, token: &str) {
        let pool = &self.served.runtime.pool;
        let leveled = sqlx::query(
            "UPDATE node SET level = $1 FROM commission c WHERE c.id = $2 AND node.id = c.node_id",
        )
        .bind(token)
        .bind(Uuid::from(commission))
        .execute(pool)
        .await
        .expect("update the commission node's level");
        assert_eq!(leveled.rows_affected(), 1, "one node is updated");
        let mirrored = sqlx::query("UPDATE commission SET visibility = $1 WHERE id = $2")
            .bind(token)
            .bind(Uuid::from(commission))
            .execute(pool)
            .await
            .expect("update the commission's own visibility column");
        assert_eq!(mirrored.rows_affected(), 1, "one commission row is updated");
    }
}

/// A commission id no seed writes.
pub fn absent_commission() -> CommissionId {
    CommissionId::from(ABSENT_COMMISSION)
}

/// A file key no seed writes.
pub fn absent_file() -> FileKey {
    FileKey::from(ABSENT_FILE)
}

/// A slot id no seed writes.
pub fn absent_slot() -> ElementId {
    ElementId::from(ABSENT_SLOT)
}

/// A Workflow id no seed writes.
pub fn absent_workflow() -> WorkflowId {
    WorkflowId::from(ABSENT_WORKFLOW)
}

/// alice's PDS profile: the fixture DID, display name "Alice", handle `alice.test`.
fn alice_profile() -> Profile {
    let did = adapter_mem::fixture::alice().did().clone();
    Profile::new(did, "alice.test").with_display_name("Alice")
}

/// bob's PDS profile: the fixture DID, handle `bob.test`, no display name.
fn bob_profile() -> Profile {
    let did = adapter_mem::fixture::bob().did().clone();
    Profile::new(did, "bob.test")
}

/// The stranger's PDS profile, handle `stranger.test`.
fn stranger_profile() -> Profile {
    let did = Did::from(STRANGER_DID.to_owned());
    Profile::new(did, "stranger.test")
}

/// Signs `profile` in, then reads `GET /me` as them, which caches the profile their root's
/// name is read from, as the web's first read after sign-in does.
async fn sign_in(served: &PgServed, profile: Profile) -> reqwest::Client {
    let client = served.sign_in(profile).await;
    let me = client
        .get(format!("{}/me", served.base_url))
        .send()
        .await
        .expect("GET /me is answered");
    assert_eq!(me.status(), 200, "GET /me after sign-in");
    client
}

/// `POST {path}` with a JSON `body` as `client`.
async fn post_json(
    client: &reqwest::Client,
    served: &PgServed,
    path: &str,
    body: &Value,
) -> reqwest::Response {
    client
        .post(format!("{}{path}", served.base_url))
        .json(body)
        .send()
        .await
        .expect("the request is answered")
}

/// A `201` answer's JSON body.
async fn created_json(response: reqwest::Response) -> Value {
    assert_eq!(response.status(), 201, "{}", response.url());
    response.json().await.expect("a JSON body")
}

/// A JSON body's string field `field`.
fn string_field<'a>(body: &'a Value, field: &str) -> &'a str {
    body[field]
        .as_str()
        .unwrap_or_else(|| panic!("the body carries a string {field}: {body}"))
}

/// Founds the Account `name` under the handle `<label>.zurfur.app` as `owner`, through
/// `POST /accounts`.
async fn found_account(
    served: &PgServed,
    owner: &reqwest::Client,
    name: &str,
    label: &str,
) -> AccountId {
    let founding = json!({ "name": name, "handle": format!("{label}.zurfur.app") });
    let response = post_json(owner, served, "/accounts", &founding).await;
    let body = created_json(response).await;
    let did = string_field(&body, "did").to_owned();
    AccountId::from(Did::from(did))
}

/// Creates the Workflow `name` on `account`'s board as `actor`, through the create use case:
/// no route creates a Workflow yet.
async fn create_workflow(
    served: &PgServed,
    actor: UserId,
    account: &AccountId,
    name: &str,
) -> WorkflowId {
    let command = application::account::workflow::create::Command {
        actor_id: actor,
        account_id: account.clone(),
        workflow_name: name.parse().expect("a valid Workflow name"),
    };
    let app = served.runtime.app();
    let accounts = app.accounts();
    let created = accounts
        .workflows()
        .create(command)
        .await
        .expect("the Workflow is created");
    created.workflow.id
}

/// Creates the commission `title` as `owner`, uploads `filename` to it and declares the
/// slot `slot_title`, each through its route.
async fn seed_commission(
    served: &PgServed,
    owner: &reqwest::Client,
    title: &str,
    filename: &str,
    slot_title: &str,
) -> SeededCommission {
    let creating = json!({ "title": title });
    let response = post_json(owner, served, "/commissions", &creating).await;
    let created = created_json(response).await;
    let id: CommissionId = string_field(&created, "id")
        .parse()
        .expect("the commission id is a UUID");

    let file = upload(served, owner, id, filename).await;
    let slot = declare_slot(served, owner, id, slot_title).await;
    SeededCommission { id, file, slot }
}

/// Uploads a small PNG named `filename` to `commission` through `POST /commissions/{id}/files`.
async fn upload(
    served: &PgServed,
    uploader: &reqwest::Client,
    commission: CommissionId,
    filename: &str,
) -> FileKey {
    let part = reqwest::multipart::Part::bytes(b"den world image bytes".to_vec())
        .file_name(filename.to_owned())
        .mime_str("image/png")
        .expect("a valid content type");
    let form = reqwest::multipart::Form::new().part("file", part);
    let response = uploader
        .post(format!(
            "{}/commissions/{commission}/files",
            served.base_url
        ))
        .multipart(form)
        .send()
        .await
        .expect("the upload is answered");
    let body = created_json(response).await;
    string_field(&body, "id")
        .parse()
        .expect("the file key is a UUID")
}

/// Declares the slot `title` on `commission`'s only surface through
/// `POST /commissions/{id}/slots`. No route hands out a tab id, so it is read from the store.
async fn declare_slot(
    served: &PgServed,
    owner: &reqwest::Client,
    commission: CommissionId,
    title: &str,
) -> ElementId {
    let composition = served
        .runtime
        .commissions
        .load_composition(&commission)
        .await
        .expect("the composition loads")
        .expect("a created commission has a composition");
    let tab = composition
        .tabs
        .first()
        .expect("a created commission has its skeleton tab");
    let slot = json!([{
        "tab": Uuid::from(tab.id),
        "surface": SKELETON[0].surfaces[0],
        "title": title,
    }]);
    let path = format!("/commissions/{commission}/slots");
    let response = post_json(owner, served, &path, &slot).await;
    let body = created_json(response).await;
    let id = body["ids"][0]
        .as_str()
        .unwrap_or_else(|| panic!("the body carries the slot's id: {body}"));
    id.parse().expect("the slot id is a UUID")
}
