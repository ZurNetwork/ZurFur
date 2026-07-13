//! ZMVP-77 — the owner declares Slots (Character positions; fill deferred) over
//! HTTP.
//!
//! Pins the acceptance criteria at the API surface (the store-layer seams are
//! covered in `adapter-mem`/`adapter-pg`):
//!
//! - **AC1** — the owner declares Slots with `POST /commissions/{id}/slots`,
//!   whose body is an **array** of Slot objects (PR #108 ruling: Slots usually
//!   arrive several at a time; the batch lands all-or-nothing, an empty array
//!   is a `422`): each with a required title (trimmed, blank refused with a
//!   `422`) and optional freeform notes (trimmed; blank normalizes to absent).
//!   Each Slot is carried by an ordinary component leaf in the tree, its
//!   title/notes riding in the satellite.
//! - **AC2** — a commission holds zero or more Slots; an empty (unfilled) Slot
//!   is a valid, permanent state — there is no occupant anywhere to be missing.
//! - **AC3** — filling is not offered: no fill surface exists on this route (or
//!   any other), and no occupant is representable in the read-back shape.
//! - The floors: anonymous is `401`; a non-participant (and a truly absent
//!   commission) gets the one uniform `commission_not_found` 404 — never a 403,
//!   and byte-identical bodies, so no existence oracle; a fabricated/foreign
//!   parent is a `node_not_found` 404; a component parent is a `409`
//!   `parent_not_a_surface`; a malformed body is a `422`. Declaring a Slot
//!   appends **no** changelog entry — the frozen ZMVP-87 taxonomy carries
//!   `seat_declared` for Seats but no Slot variant.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle, NodeId, NodeKind},
    did::Did,
    profile::Profile,
    user::User,
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] so a test can introspect the tree and slots that were
/// persisted. `did` is the identity `sign_in` will authenticate as.
async fn spawn_app(did: &str) -> (String, MemBackend) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let backend = MemBackend::new();
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
        database: backend.database(),
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        files: backend.file_store(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), backend)
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

/// Creates a commission over HTTP as the signed-in caller and returns its id
/// (introspected off the backend — the route returns a bare `201`).
async fn create_commission(
    client: &reqwest::Client,
    base: &str,
    backend: &MemBackend,
) -> uuid::Uuid {
    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "A ref sheet" }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201, "creating a commission returns 201");
    let all = backend.all_commissions().await.expect("list commissions");
    *all.last().expect("a commission was persisted").id
}

/// The commission's root node id, introspected off the backend.
async fn root_of(backend: &MemBackend, commission: uuid::Uuid) -> uuid::Uuid {
    *backend
        .commission_store()
        .load_tree(CommissionId::new(commission))
        .await
        .expect("load tree")
        .expect("every commission has a tree")
        .root
        .id
}

/// POSTs a Slot-declaration batch (a JSON array of Slot objects) and returns
/// the carrying components' node ids from the `201` body, in request order.
async fn declare_slots(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    body: &serde_json::Value,
) -> Vec<uuid::Uuid> {
    let res = client
        .post(format!("{base}/commissions/{commission}/slots"))
        .json(body)
        .send()
        .await
        .expect("POST slots");
    assert_eq!(res.status(), 201, "declaring slots returns 201");
    let body: serde_json::Value = res.json().await.expect("201 body is JSON");
    body["ids"]
        .as_array()
        .expect("the body carries the new node ids")
        .iter()
        .map(|id| {
            id.as_str()
                .expect("each id is a string")
                .parse()
                .expect("each id is a UUID")
        })
        .collect()
}

/// Seeds a committed commission owned by a directly-provisioned user (someone
/// other than the signed-in caller), returning its id.
async fn seed_foreign_commission(backend: &MemBackend) -> uuid::Uuid {
    let owner: User = backend
        .provision(&Did::new("did:plc:someone-else".to_string()))
        .await
        .expect("provision foreign owner");
    let title = CommissionTitle::try_new("Not yours").expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    let id = *commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed foreign commission");
    id
}

// AC1/AC2 — the owner declares Slots under the root and under a nested surface:
// each lands as a component leaf (no mode, empty payload, the owner's envelope)
// whose satellite carries the trimmed title and the notes (present on one,
// absent on the other) — a commission going from zero Slots to two. No
// changelog entry is appended (the frozen taxonomy has no Slot variant).
#[tokio::test]
async fn the_owner_declares_slots_with_title_and_optional_notes() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    // Zero Slots is a valid state (AC2).
    assert!(
        backend
            .slots_of(CommissionId::new(id))
            .await
            .expect("list slots")
            .is_empty(),
        "a fresh commission holds zero Slots"
    );

    // Grow a nested surface over HTTP to prove slots attach under any surface.
    let res = client
        .post(format!("{base}/commissions/{id}/surfaces"))
        .json(&json!({ "parent": root }))
        .send()
        .await
        .expect("POST surface");
    assert_eq!(res.status(), 201);
    let surface: uuid::Uuid = res.json::<serde_json::Value>().await.expect("body")["id"]
        .as_str()
        .expect("id")
        .parse()
        .expect("uuid");

    // One request declares both (the array contract, PR #108 ruling); the 201
    // ids come back in request order.
    let ids = declare_slots(
        &client,
        &base,
        id,
        &json!([
            { "parent": root, "title": "  The knight  ", "notes": "  full plate, no cape  " },
            { "parent": surface, "title": "The mage" },
        ]),
    )
    .await;
    let (noted, bare) = (ids[0], ids[1]);

    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    // The tree half: both slots are ordinary component leaves.
    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 2);
    let on_root = &tree.root.children[1];
    assert_eq!(*on_root.id, noted, "the 201 id reappears in the tree");
    assert!(
        matches!(on_root.kind, NodeKind::Component),
        "the Slot's carrying node is a component (no mode of its own)"
    );
    assert_eq!(on_root.created_by, me.id, "the envelope names the creator");
    assert_eq!(
        on_root.payload,
        json!({}),
        "the carrying component's payload is empty — the Slot lives in the satellite"
    );
    assert!(
        on_root.children.is_empty(),
        "the carrying component is a leaf"
    );
    assert_eq!(
        *tree.root.children[0].children[0].id, bare,
        "Slots grow under non-root surfaces too"
    );

    // The satellite half: trimmed title, notes present/absent as declared.
    let noted_slot = backend
        .find_slot(NodeId::new(noted))
        .await
        .expect("find slot")
        .expect("the declared slot has its satellite");
    assert_eq!(noted_slot.title.as_str(), "The knight", "title is trimmed");
    assert_eq!(
        noted_slot.notes.as_deref(),
        Some("full plate, no cape"),
        "notes are trimmed and kept"
    );
    assert_eq!(noted_slot.commission_id, CommissionId::new(id));

    let bare_slot = backend
        .find_slot(NodeId::new(bare))
        .await
        .expect("find slot")
        .expect("satellite exists");
    assert_eq!(bare_slot.title.as_str(), "The mage");
    assert!(bare_slot.notes.is_none(), "omitted notes stay absent");

    // Zero or more: the commission now counts exactly two (AC2).
    let slots = backend
        .slots_of(CommissionId::new(id))
        .await
        .expect("list slots");
    assert_eq!(slots.len(), 2, "the commission holds two declared Slots");

    // Declaring Slots appends NO changelog entry (the taxonomy's seat_declared
    // is seat-specific; no Slot variant exists): only creation is in the stream.
    let entries = backend
        .changelog_entries(CommissionId::new(id))
        .await
        .expect("changelog");
    assert_eq!(
        entries.len(),
        1,
        "slot declaration is not changelog-recorded"
    );
}

// AC1 — the title is required: a blank title (and a missing one) is a 422, and
// nothing lands — no node, no satellite.
#[tokio::test]
async fn a_blank_or_missing_title_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "parent": root, "title": "   " }]))
        .send()
        .await
        .expect("POST blank title");
    common::assert_problem(res, 422, "invalid_request").await;

    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "parent": root }]))
        .send()
        .await
        .expect("POST missing title");
    common::assert_problem(res, 422, "invalid_request").await;

    // Declaring nothing is malformed, not a no-op (the array contract).
    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([]))
        .send()
        .await
        .expect("POST empty batch");
    common::assert_problem(res, 422, "invalid_request").await;

    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert!(tree.root.children.is_empty(), "no refused write landed");
    assert!(
        backend
            .slots_of(CommissionId::new(id))
            .await
            .expect("list slots")
            .is_empty()
    );
}

// AC1 — notes are optional freeform: blank notes normalize to absent rather
// than storing whitespace.
#[tokio::test]
async fn blank_notes_normalize_to_absent() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let node = declare_slots(
        &client,
        &base,
        id,
        &json!([{ "parent": root, "title": "The bard", "notes": "   " }]),
    )
    .await[0];

    let slot = backend
        .find_slot(NodeId::new(node))
        .await
        .expect("find slot")
        .expect("satellite exists");
    assert!(slot.notes.is_none(), "blank notes are not stored");
}

// AC1 — declaring under a component (a Slot's own carrying component included)
// is rejected with a 409 parent_not_a_surface, and nothing lands.
#[tokio::test]
async fn declaring_under_a_component_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;
    let slot = declare_slots(
        &client,
        &base,
        id,
        &json!([{ "parent": root, "title": "First" }]),
    )
    .await[0];

    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "parent": slot, "title": "Nested?" }]))
        .send()
        .await
        .expect("POST slot under slot");
    common::assert_problem(res, 409, "parent_not_a_surface").await;

    // All-or-nothing: a refusing slot mid-batch takes the valid one with it.
    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([
            { "parent": root, "title": "Would be fine alone" },
            { "parent": slot, "title": "Nested?" },
        ]))
        .send()
        .await
        .expect("POST mixed batch");
    common::assert_problem(res, 409, "parent_not_a_surface").await;

    let slots = backend
        .slots_of(CommissionId::new(id))
        .await
        .expect("list slots");
    assert_eq!(
        slots.len(),
        1,
        "only the first slot exists — no refused batch left its valid half behind"
    );
}

// Floor — anonymous callers can't declare slots: 401.
#[tokio::test]
async fn an_anonymous_caller_cannot_declare_a_slot() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let res = client()
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "parent": root, "title": "The knight" }]))
        .send()
        .await
        .expect("anonymous POST");
    common::assert_problem(res, 401, "not_authenticated").await;
}

// Floor (the closed door) — a signed-in NON-participant probing someone else's
// commission gets the one uniform commission_not_found 404, byte-identical to
// the answer for a commission that does not exist at all. Never a 403.
#[tokio::test]
async fn a_non_participant_gets_the_uniform_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;
    let foreign_root = root_of(&backend, foreign).await;

    let hidden = client
        .post(format!("{base}/commissions/{foreign}/slots"))
        .json(&json!([{ "parent": foreign_root, "title": "Probe" }]))
        .send()
        .await
        .expect("probe foreign");
    let hidden_status = hidden.status().as_u16();
    let hidden_body: serde_json::Value = hidden.json().await.expect("problem body");

    let absent_id = uuid::Uuid::now_v7();
    let absent = client
        .post(format!("{base}/commissions/{absent_id}/slots"))
        .json(&json!([{ "parent": foreign_root, "title": "Probe" }]))
        .send()
        .await
        .expect("probe absent");
    let absent_status = absent.status().as_u16();
    let absent_body: serde_json::Value = absent.json().await.expect("problem body");

    assert_eq!(hidden_status, 404, "hidden = not found, never 403");
    assert_eq!(hidden_body["code"], "commission_not_found");
    assert_eq!(
        (hidden_status, &hidden_body),
        (absent_status, &absent_body),
        "hidden and absent are indistinguishable (no existence oracle)"
    );

    assert!(
        backend
            .slots_of(CommissionId::new(foreign))
            .await
            .expect("list slots")
            .is_empty(),
        "the probe wrote nothing"
    );
}

// Floor — the owner naming a parent node that doesn't exist in this commission
// (fabricated, or belonging to another tree) gets node_not_found; the foreign
// case answers identically to the fabricated one.
#[tokio::test]
async fn an_unknown_or_foreign_parent_is_node_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "parent": uuid::Uuid::now_v7(), "title": "The knight" }]))
        .send()
        .await
        .expect("POST fabricated parent");
    common::assert_problem(res, 404, "node_not_found").await;

    let foreign = seed_foreign_commission(&backend).await;
    let foreign_root = root_of(&backend, foreign).await;
    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "parent": foreign_root, "title": "The knight" }]))
        .send()
        .await
        .expect("POST foreign parent");
    common::assert_problem(res, 404, "node_not_found").await;
}

// Floor — a malformed body (no parent) is a 422.
#[tokio::test]
async fn a_malformed_body_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "title": "The knight" }]))
        .send()
        .await
        .expect("POST malformed");
    common::assert_problem(res, 422, "invalid_request").await;
}
