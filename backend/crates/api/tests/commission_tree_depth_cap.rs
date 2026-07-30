//! ZMVP-164 — the write-side surface-tree depth cap, pinned at every
//! tree-mutating route (Surface Tree on the Wire DD `42762241`'s companion
//! ruling).
//!
//! The domain counts a node's own nesting level with the root surface at
//! level **1** (see [`domain::elements::commission::MAX_SURFACE_TREE_DEPTH`]),
//! so it lines up 1:1 with the wire `SurfaceTree`/`SurfaceNode` nesting the DD
//! mints. Rust's `prost` decoder dies at 64 nested levels, so the write path
//! must refuse anything that would land a node at level 64 — the boundary
//! this suite pins, at **every** route that grows the tree:
//!
//! - `POST /commissions/{id}/surfaces` (ZMVP-71)
//! - `POST /commissions/{id}/components` (ZMVP-72)
//! - `POST /commissions/{id}/seats` (ZMVP-76)
//! - `POST /commissions/{id}/slots` (ZMVP-77)
//!
//! Each route shares ONE enforced gate
//! (`PgCommissionWrites::require_surface_parent` / its `adapter-mem` mirror),
//! so pinning the boundary once per route proves the shared guard, not four
//! independent implementations. A node at depth 63 is always accepted; one
//! that would land at depth 64 is always refused with the `422`
//! `tree_depth_exceeded` problem.
//!
//! Same in-process fakes as the other api e2e suites — no network, no
//! database (the write-side depth column and its `CHECK` backstop are pinned
//! separately against real PostgreSQL in `adapter-pg`'s own integration
//! tests).

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use domain::elements::{commission::CommissionId, did::Did, profile::Profile};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

/// Boots the app with everything faked in-process; returns the base URL and
/// the [`MemBackend`] so a test can introspect the tree that was persisted.
/// `did` is the identity `sign_in` will authenticate as.
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

/// Drives the two-step sign-in so the client's cookie jar carries a live
/// session for the app's configured DID.
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

/// The commission's root node id, introspected off the backend — nesting
/// level **1** by this ticket's depth convention.
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

/// POSTs a new surface under `parent` and returns the raw response, so a
/// caller can assert either the `201` accept case or a refusal.
async fn post_surface(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    parent: uuid::Uuid,
) -> reqwest::Response {
    client
        .post(format!("{base}/commissions/{commission}/surfaces"))
        .json(&json!({ "parent": parent }))
        .send()
        .await
        .expect("POST surface")
}

/// POSTs a new surface under `parent`, asserts the `201` accept case, and
/// returns the created node's id from the body — the chain-builder's
/// workhorse.
async fn add_surface(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    parent: uuid::Uuid,
) -> uuid::Uuid {
    let res = post_surface(client, base, commission, parent).await;
    assert_eq!(res.status(), 201, "adding a surface returns 201");
    let body: serde_json::Value = res.json().await.expect("201 body is JSON");
    body["id"]
        .as_str()
        .expect("the body carries the new node id")
        .parse()
        .expect("the id is a UUID")
}

/// Grows a straight-line surface chain from `root` (nesting level **1**) down
/// to `target_depth`, one `add_surface` call per level — each call is itself
/// an acceptance pin for every level it passes through. Returns the node id
/// at *every* level, indexed so `chain[level - 1]` is that level's id
/// (`chain[0] == root`); `chain.len() == target_depth`.
async fn surface_chain_to_depth(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    root: uuid::Uuid,
    target_depth: usize,
) -> Vec<uuid::Uuid> {
    let mut chain = vec![root];
    while chain.len() < target_depth {
        let parent = *chain.last().expect("the chain is never empty");
        let child = add_surface(client, base, commission, parent).await;
        chain.push(child);
    }
    chain
}

// A surface may be added at depth 63 (the deepest level a write may ever
// produce — one below Rust's 64-level prost decode ceiling); one more level,
// depth 64, is refused with the 422 tree_depth_exceeded problem, and nothing
// is written.
#[tokio::test]
async fn surfaces_accept_depth_63_and_reject_depth_64() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    // The chain's last `add_surface` call IS the depth-63 acceptance: parent
    // depth 62 -> child depth 63.
    let chain = surface_chain_to_depth(&client, &base, id, root, 63).await;
    let depth_63 = *chain.last().expect("chain reaches depth 63");

    let res = post_surface(&client, &base, id, depth_63).await;
    common::assert_problem(res, 422, "tree_depth_exceeded").await;

    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    let mut node = &tree.root;
    for expected_child in &chain[1..] {
        assert_eq!(node.children.len(), 1, "a straight-line chain");
        node = &node.children[0];
        assert_eq!(*node.id, *expected_child);
    }
    assert!(
        node.children.is_empty(),
        "the refused depth-64 surface was never written"
    );
}

// A component may land at depth 63 but not depth 64 — the leaf mirror of the
// surface boundary, behind the SAME shared parent gate.
#[tokio::test]
async fn components_accept_depth_63_and_reject_depth_64() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let chain = surface_chain_to_depth(&client, &base, id, root, 62).await;
    let depth_62 = *chain.last().expect("chain reaches depth 62");
    let depth_63 = add_surface(&client, &base, id, depth_62).await;

    // Accept: a component under the depth-62 surface lands at depth 63.
    let res = client
        .post(format!("{base}/commissions/{id}/components"))
        .json(&json!({ "parent": depth_62 }))
        .send()
        .await
        .expect("POST component");
    assert_eq!(
        res.status(),
        201,
        "a component at depth 63 (parent depth 62) is accepted"
    );

    // Reject: a component under the depth-63 surface would land at depth 64.
    let res = client
        .post(format!("{base}/commissions/{id}/components"))
        .json(&json!({ "parent": depth_63 }))
        .send()
        .await
        .expect("POST component");
    common::assert_problem(res, 422, "tree_depth_exceeded").await;
}

// A declared Seat may land at depth 63 but not depth 64 — the same shared
// parent gate, exercised through the Seat's dedicated node+satellite write.
#[tokio::test]
async fn seats_accept_depth_63_and_reject_depth_64() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let chain = surface_chain_to_depth(&client, &base, id, root, 62).await;
    let depth_62 = *chain.last().expect("chain reaches depth 62");
    let depth_63 = add_surface(&client, &base, id, depth_62).await;

    // Accept: a seat under the depth-62 surface lands at depth 63.
    let res = client
        .post(format!("{base}/commissions/{id}/seats"))
        .json(&json!({ "parent": depth_62, "kind": "Creator" }))
        .send()
        .await
        .expect("POST seat");
    assert_eq!(
        res.status(),
        201,
        "a seat at depth 63 (parent depth 62) is accepted"
    );

    // Reject: a seat under the depth-63 surface would land at depth 64.
    let res = client
        .post(format!("{base}/commissions/{id}/seats"))
        .json(&json!({ "parent": depth_63, "kind": "Creator" }))
        .send()
        .await
        .expect("POST seat");
    common::assert_problem(res, 422, "tree_depth_exceeded").await;
}

// A declared Slot may land at depth 63 but not depth 64 — the same shared
// parent gate, exercised through the array-bodied declare-Slots route.
#[tokio::test]
async fn slots_accept_depth_63_and_reject_depth_64() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let chain = surface_chain_to_depth(&client, &base, id, root, 62).await;
    let depth_62 = *chain.last().expect("chain reaches depth 62");
    let depth_63 = add_surface(&client, &base, id, depth_62).await;

    // Accept: a slot under the depth-62 surface lands at depth 63.
    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "parent": depth_62, "title": "Character A" }]))
        .send()
        .await
        .expect("POST slots");
    assert_eq!(
        res.status(),
        201,
        "a slot at depth 63 (parent depth 62) is accepted"
    );

    // Reject: a slot under the depth-63 surface would land at depth 64 — and
    // the whole batch is refused (all-or-nothing), not just this entry.
    let res = client
        .post(format!("{base}/commissions/{id}/slots"))
        .json(&json!([{ "parent": depth_63, "title": "Character B" }]))
        .send()
        .await
        .expect("POST slots");
    common::assert_problem(res, 422, "tree_depth_exceeded").await;
}
