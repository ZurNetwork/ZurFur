//! ZMVP-73 — the owner removes a node and its subtree over HTTP.
//!
//! Pins the acceptance criteria at the API surface (the store-layer seams are
//! covered in `adapter-mem`/`adapter-pg`):
//!
//! - **AC1** — the owner removes a surface with
//!   `DELETE /commissions/{id}/nodes/{node}`; its **entire subtree** goes with
//!   it (nested surfaces and components alike), and the remaining siblings keep
//!   a consistent order.
//! - **AC2** — the owner removes a component singly (a leaf: just it).
//! - **AC3** — the root surface cannot be removed: a `409` `cannot_remove_root`
//!   problem, tree untouched. The Title is not a tree node at all — no node id
//!   addresses it, so it is irremovable by construction rather than by check.
//! - The floors: anonymous is `401`; a non-participant (and a truly absent
//!   commission) gets the one uniform `commission_not_found` 404 — never a 403,
//!   and byte-identical bodies, so no existence oracle; a fabricated node id
//!   and a node in someone else's tree are one indistinguishable
//!   `node_not_found` 404. Tree edits append **no** changelog entries (not in
//!   the frozen taxonomy).
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionId, CommissionTitle},
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

/// POSTs a new component under `parent` and returns the created node's id from
/// the `201` body.
async fn add_component(
    client: &reqwest::Client,
    base: &str,
    commission: uuid::Uuid,
    parent: uuid::Uuid,
) -> uuid::Uuid {
    let res = client
        .post(format!("{base}/commissions/{commission}/components"))
        .json(&json!({ "parent": parent, "payload": { "kind": "text" } }))
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
    let title = CommissionTitle::try_new("Not yours").expect("valid title");
    let commission = Commission::create(title, owner.id, Utc::now(), None);
    let id = *commission.id;
    backend
        .create_commission(&commission)
        .await
        .expect("seed foreign commission");
    id
}

// AC1 — the owner removes a mid-tree surface; the ENTIRE subtree under it (a
// nested surface and components) goes with it, the untouched siblings survive
// in order, and no changelog entry is appended (the taxonomy is frozen).
#[tokio::test]
async fn the_owner_removes_a_surface_and_its_whole_subtree() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    // root -> [first, doomed, last]; under doomed: a component and a surface
    // that itself holds a component.
    let first = add_surface(&client, &base, id, root).await;
    let doomed = add_surface(&client, &base, id, root).await;
    let last = add_surface(&client, &base, id, root).await;
    add_component(&client, &base, id, doomed).await;
    let nested = add_surface(&client, &base, id, doomed).await;
    add_component(&client, &base, id, nested).await;

    let res = client
        .delete(format!("{base}/commissions/{id}/nodes/{doomed}"))
        .send()
        .await
        .expect("DELETE surface");
    assert_eq!(res.status(), 204, "removal answers 204 No Content");

    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert_eq!(
        tree.root.children.len(),
        2,
        "the surface and its whole subtree are gone"
    );
    assert_eq!(
        *tree.root.children[0].id, first,
        "siblings keep their order"
    );
    assert_eq!(*tree.root.children[1].id, last);
    assert!(
        tree.root.children.iter().all(|c| c.children.is_empty()),
        "nothing of the removed subtree survives anywhere"
    );

    let entries = backend
        .changelog_entries(CommissionId::new(id))
        .await
        .expect("changelog");
    assert_eq!(
        entries.len(),
        1,
        "removal appends no changelog entry (only creation is recorded)"
    );
}

// AC2 — the owner removes a component: just that leaf goes; its parent surface
// and the component's siblings survive in order.
#[tokio::test]
async fn the_owner_removes_a_component_singly() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let doomed = add_component(&client, &base, id, root).await;
    let surface = add_surface(&client, &base, id, root).await;
    let kept = add_component(&client, &base, id, root).await;

    let res = client
        .delete(format!("{base}/commissions/{id}/nodes/{doomed}"))
        .send()
        .await
        .expect("DELETE component");
    assert_eq!(res.status(), 204);

    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 2, "only the one leaf went");
    assert_eq!(*tree.root.children[0].id, surface, "order holds");
    assert_eq!(*tree.root.children[1].id, kept);
}

// AC3 — the root surface cannot be removed: a 409 cannot_remove_root problem,
// and the tree is untouched. (The Title is not a tree node — no node id
// addresses it, so there is nothing to even aim this route at.)
#[tokio::test]
async fn the_root_surface_cannot_be_removed() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;
    let child = add_surface(&client, &base, id, root).await;

    let res = client
        .delete(format!("{base}/commissions/{id}/nodes/{root}"))
        .send()
        .await
        .expect("DELETE root");
    common::assert_problem(res, 409, "cannot_remove_root").await;

    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert_eq!(*tree.root.id, root, "the root survives");
    assert_eq!(*tree.root.children[0].id, child, "and so does its subtree");
}

// Floor — anonymous callers can't remove nodes: 401, and nothing goes.
#[tokio::test]
async fn an_anonymous_caller_cannot_remove_a_node() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend).await;
    let root = root_of(&backend, id).await;
    let surface = add_surface(&signed_in, &base, id, root).await;

    let res = client()
        .delete(format!("{base}/commissions/{id}/nodes/{surface}"))
        .send()
        .await
        .expect("anonymous DELETE");
    common::assert_problem(res, 401, "not_authenticated").await;

    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 1, "nothing was removed");
}

// Floor (the closed door) — a signed-in NON-participant probing someone else's
// commission gets the one uniform commission_not_found 404, byte-identical to
// the answer for a commission that does not exist at all. Never a 403 — and
// nothing is removed.
#[tokio::test]
async fn a_non_participant_gets_the_uniform_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;
    let foreign_root = root_of(&backend, foreign).await;

    // Probing a real commission I may not see...
    let hidden = client
        .delete(format!("{base}/commissions/{foreign}/nodes/{foreign_root}"))
        .send()
        .await
        .expect("probe foreign");
    let hidden_status = hidden.status().as_u16();
    let hidden_body: serde_json::Value = hidden.json().await.expect("problem body");

    // ...answers exactly like probing one that doesn't exist.
    let absent_id = uuid::Uuid::now_v7();
    let absent = client
        .delete(format!(
            "{base}/commissions/{absent_id}/nodes/{foreign_root}"
        ))
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

    // And the probe removed nothing.
    let tree = backend
        .commission_store()
        .load_tree(CommissionId::new(foreign))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert_eq!(*tree.root.id, foreign_root, "the foreign root survives");
}

// Floor — the owner aiming at a node that doesn't exist in this commission
// (fabricated, or belonging to another tree) gets node_not_found; the foreign
// case answers identically to the fabricated one — never cannot_remove_root,
// which would leak what a foreign node is — and the foreign tree is untouched.
#[tokio::test]
async fn an_unknown_or_foreign_node_is_node_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    // Fabricated node id.
    let res = client
        .delete(format!(
            "{base}/commissions/{id}/nodes/{}",
            uuid::Uuid::now_v7()
        ))
        .send()
        .await
        .expect("DELETE fabricated node");
    common::assert_problem(res, 404, "node_not_found").await;

    // A real node — someone else's ROOT, addressed through my own commission:
    // still just node_not_found (not cannot_remove_root), and it survives.
    let foreign = seed_foreign_commission(&backend).await;
    let foreign_root = root_of(&backend, foreign).await;
    let res = client
        .delete(format!("{base}/commissions/{id}/nodes/{foreign_root}"))
        .send()
        .await
        .expect("DELETE foreign node");
    common::assert_problem(res, 404, "node_not_found").await;

    assert_eq!(
        root_of(&backend, foreign).await,
        foreign_root,
        "the foreign tree is untouched"
    );
}
