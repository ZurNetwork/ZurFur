//! ZMVP-72 — the owner adds components (the tree's leaves) over HTTP.
//!
//! Pins the acceptance criteria at the API surface (the store-layer seams are
//! covered in `adapter-mem`/`adapter-pg`):
//!
//! - **AC1** — the owner adds a component under any surface (root and
//!   non-root) with `POST /commissions/{id}/components`; adding one under a
//!   component is rejected with a `409` `parent_not_a_surface` problem — and so
//!   is adding a *surface* under a component (components never have children).
//! - **AC2** — a component has no children and carries no visibility mode of
//!   its own — it projects with its parent (no mode is accepted from the
//!   client, and none is even representable on the kind).
//! - **AC3** — a component holds an envelope and an opaque payload; the
//!   payload round-trips unmodified (asserted off the loaded tree).
//! - The floors: anonymous is `401`; a non-participant (and a truly absent
//!   commission) gets the one uniform `commission_not_found` 404 — never a 403,
//!   and byte-identical bodies, so no existence oracle; a fabricated/foreign
//!   parent is a `node_not_found` 404; a malformed body is a `422`. Tree edits
//!   append **no** changelog entries (not in the frozen taxonomy).
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle, NodeKind},
    did::Did,
    profile::Profile,
    user::User,
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] so a test can introspect the tree that was persisted. `did` is
/// the identity `sign_in` will authenticate as.
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
        database: backend.database(),
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
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

/// POSTs a new surface under `parent` and returns the created node's id from
/// the `201` body.
async fn add_surface(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    parent: uuid::Uuid,
) -> uuid::Uuid {
    let res = client
        .post(format!("{base}/commissions/{commission}/surfaces"))
        .json(&json!({ "parent": parent }))
        .send()
        .await
        .expect("POST surface");
    assert_eq!(res.status(), 201, "adding a surface returns 201");
    let body: serde_json::Value = res.json().await.expect("201 body is JSON");
    body["id"]
        .as_str()
        .expect("the body carries the new node id")
        .parse()
        .expect("the id is a UUID")
}

/// POSTs a new component under `parent` carrying `payload` and returns the
/// created node's id from the `201` body.
async fn add_component(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    parent: uuid::Uuid,
    payload: &serde_json::Value,
) -> uuid::Uuid {
    let res = client
        .post(format!("{base}/commissions/{commission}/components"))
        .json(&json!({ "parent": parent, "payload": payload }))
        .send()
        .await
        .expect("POST component");
    assert_eq!(res.status(), 201, "adding a component returns 201");
    let body: serde_json::Value = res.json().await.expect("201 body is JSON");
    body["id"]
        .as_str()
        .expect("the body carries the new node id")
        .parse()
        .expect("the id is a UUID")
}

/// Seeds a committed commission owned by a directly-provisioned user (someone
/// other than the signed-in caller), returning its id.
async fn seed_foreign_commission(backend: &MemBackend) -> uuid::Uuid {
    let owner: User = backend
        .provision(&Did::new("did:plc:someone-else".to_string()))
        .await
        .expect("provision foreign owner");
    let title = "Not yours".parse::<CommissionTitle>().expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    let id = *commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed foreign commission");
    id
}

// AC1/AC2/AC3 — the owner adds components under the root and under a nested
// surface; each lands as a leaf of kind Component (no mode is representable),
// carries the creator's envelope, and its payload — nested structure, unicode,
// numbers, booleans, in-payload nulls — reads back exactly as sent.
#[tokio::test]
async fn the_owner_adds_components_under_any_surface() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;
    let surface = add_surface(&client, &base, id, root).await;

    let payload = json!({
        "kind": "text",
        "body": "Reference: 三毛猫 🐾 — \"line\\break\"",
        "revision": 3,
        "ratio": 1.5,
        "flags": [true, false, null],
        "nested": { "empty": {}, "list": [] },
    });
    let on_root = add_component(&client, &base, id, root, &payload).await;
    let nested = add_component(&client, &base, id, surface, &json!({})).await;

    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 2);
    assert_eq!(*tree.root.children[0].id, surface, "append order");
    let component = &tree.root.children[1];
    assert_eq!(*component.id, on_root, "the 201 id reappears in the tree");
    assert!(
        matches!(component.kind, NodeKind::Component),
        "a component carries no mode of its own (AC2)"
    );
    assert_eq!(
        component.created_by, me.id,
        "the envelope names the creator"
    );
    assert_eq!(
        component.payload, payload,
        "the payload round-trips unmodified (AC3)"
    );
    assert!(component.children.is_empty(), "a component is a leaf (AC2)");
    assert_eq!(
        *tree.root.children[0].children[0].id, nested,
        "components grow under non-root surfaces too (AC1: any surface)"
    );

    // Tree edits are NOT changelog events (the taxonomy is frozen; ZMVP-87):
    // the stream still holds only the creation entry.
    let entries = backend
        .changelog_entries(CommissionId::new(id))
        .await
        .expect("changelog");
    assert_eq!(
        entries.len(),
        1,
        "adding components appends no changelog entry"
    );
}

// AC3 — a request that omits the payload creates a component with the empty
// object payload (the untyped v1 default), not an error.
#[tokio::test]
async fn an_omitted_payload_defaults_to_the_empty_object() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let res = client
        .post(format!("{base}/commissions/{id}/components"))
        .json(&json!({ "parent": root }))
        .send()
        .await
        .expect("POST component without payload");
    assert_eq!(res.status(), 201);

    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert_eq!(tree.root.children[0].payload, json!({}));
}

// AC1 — adding under a component is rejected: a component parent answers a 409
// parent_not_a_surface problem, for a new component AND for a new surface
// (components never have children, AC2) — and nothing lands.
#[tokio::test]
async fn adding_under_a_component_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;
    let component = add_component(&client, &base, id, root, &json!({"kind": "text"})).await;

    let res = client
        .post(format!("{base}/commissions/{id}/components"))
        .json(&json!({ "parent": component }))
        .send()
        .await
        .expect("POST component under component");
    common::assert_problem(res, 409, "parent_not_a_surface").await;

    let res = client
        .post(format!("{base}/commissions/{id}/surfaces"))
        .json(&json!({ "parent": component }))
        .send()
        .await
        .expect("POST surface under component");
    common::assert_problem(res, 409, "parent_not_a_surface").await;

    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 1);
    assert!(
        tree.root.children[0].children.is_empty(),
        "a component never has children"
    );
}

// Floor — anonymous callers can't add components: 401, and nothing lands.
#[tokio::test]
async fn an_anonymous_caller_cannot_add_a_component() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let res = client()
        .post(format!("{base}/commissions/{id}/components"))
        .json(&json!({ "parent": root }))
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

    // Probing a real commission I may not see...
    let hidden = client
        .post(format!("{base}/commissions/{foreign}/components"))
        .json(&json!({ "parent": foreign_root }))
        .send()
        .await
        .expect("probe foreign");
    let hidden_status = hidden.status().as_u16();
    let hidden_body: serde_json::Value = hidden.json().await.expect("problem body");

    // ...answers exactly like probing one that doesn't exist.
    let absent_id = uuid::Uuid::now_v7();
    let absent = client
        .post(format!("{base}/commissions/{absent_id}/components"))
        .json(&json!({ "parent": foreign_root }))
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

    // And the probe wrote nothing.
    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(foreign))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert!(tree.root.children.is_empty());
}

// Floor — the owner naming a parent node that doesn't exist in this commission
// (fabricated, or belonging to another tree) gets node_not_found; the foreign
// case answers identically to the fabricated one — never parent_not_a_surface,
// which would leak what a foreign node is.
#[tokio::test]
async fn an_unknown_or_foreign_parent_is_node_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    // Fabricated parent id.
    let res = client
        .post(format!("{base}/commissions/{id}/components"))
        .json(&json!({ "parent": uuid::Uuid::now_v7() }))
        .send()
        .await
        .expect("POST fabricated parent");
    common::assert_problem(res, 404, "node_not_found").await;

    // A real node — in someone else's tree.
    let foreign = seed_foreign_commission(&backend).await;
    let foreign_root = root_of(&backend, foreign).await;
    let res = client
        .post(format!("{base}/commissions/{id}/components"))
        .json(&json!({ "parent": foreign_root }))
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
        .post(format!("{base}/commissions/{id}/components"))
        .json(&json!({ "payload": {"kind": "text"} }))
        .send()
        .await
        .expect("POST malformed");
    common::assert_problem(res, 422, "invalid_request").await;
}
