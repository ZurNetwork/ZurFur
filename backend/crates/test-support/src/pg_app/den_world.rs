use axum::Router;
use chrono::Utc;
use composition::Runtime;
use domain::elements::{
    account::AccountId,
    commission::{CommissionId, ElementId, FileKey, SKELETON, Visibility},
    did::Did,
    profile::Profile,
    user::UserId,
    workflow::WorkflowId,
};
use serde_json::{Value, json};
use uuid::Uuid;

use super::{PgServed, serve};

/// The stranger's DID: a user who signs in and takes part in nothing.
const STRANGER_DID: &str = "did:plc:s5trstrangerfixtureaaaaa";

/// The visibility token no parser reads, for the row a stored token can't be parsed from.
const UNPARSABLE_VISIBILITY: &str = "not-a-visibility";

/// A commission id no seed writes: a UUIDv7 from 2025, before any row's.
const ABSENT_COMMISSION: Uuid = Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0a01);
/// A file key no seed writes.
const ABSENT_FILE: Uuid = Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0a02);
/// A slot id no seed writes.
const ABSENT_SLOT: Uuid = Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0a03);
/// A Workflow id no seed writes.
const ABSENT_WORKFLOW: Uuid = Uuid::from_u128(0x0193_0000_0000_7000_8000_0000_0000_0a04);

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
/// "Untitled", and any visibility other than Private ([`DenWorld::set_visibility`]).
#[expect(
    clippy::let_and_return,
    reason = "the style rulebook names a multi-line construction before returning it"
)]
pub async fn den_world(app: impl FnOnce(Runtime) -> Router) -> DenWorld {
    let served = serve(app).await;
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

    /// Sets `commission`'s stored visibility, a test-only update: no route sets it yet.
    pub async fn set_visibility(&self, commission: CommissionId, visibility: Visibility) {
        self.store_visibility(commission, &visibility.to_string())
            .await;
    }

    /// Stores a visibility token no parser reads on `commission`'s row, a test-only update.
    pub async fn store_unparsable_visibility(&self, commission: CommissionId) {
        self.store_visibility(commission, UNPARSABLE_VISIBILITY)
            .await;
    }

    /// Writes `token` as `commission`'s visibility.
    async fn store_visibility(&self, commission: CommissionId, token: &str) {
        let updated = sqlx::query("UPDATE commission SET visibility = $1 WHERE id = $2")
            .bind(token)
            .bind(Uuid::from(commission))
            .execute(&self.served.runtime.pool)
            .await
            .expect("update the commission's visibility");
        assert_eq!(updated.rows_affected(), 1, "one commission row is updated");
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
