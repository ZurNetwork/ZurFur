//! ZMVP-71 — the owner grows the commission's tree over HTTP.
//!
//! Pins the acceptance criteria at the API surface (the store-layer seams are
//! covered in `adapter-mem`/`adapter-pg`):
//!
//! - **AC1** — every commission is born with a root surface (introspected off
//!   the backend — creation itself mints it). It cannot be removed: no removal
//!   route exists at all until ZMVP-73, which ships the guarded exception.
//! - **AC2** — the owner adds a surface under any existing surface (root and
//!   non-root) with `POST /commissions/{id}/surfaces`; the `201` body carries
//!   the new node's id.
//! - **AC3** — every added surface is born mode `Total` (no mode is accepted
//!   from the client at all; widening is ZMVP-74).
//! - **AC4** — nodes carry the core-owned envelope (id, type, created_by,
//!   created_at, mode), asserted off the loaded tree.
//! - The floors: anonymous is `401`; a non-participant (and a truly absent
//!   commission) gets the one uniform `commission_not_found` 404 — never a 403,
//!   and byte-identical bodies, so no existence oracle; a fabricated parent is
//!   a `node_not_found` 404; a malformed body is a `422`. Tree edits append
//!   **no** changelog entries (not in the frozen taxonomy).
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionTitle, NodeKind, SurfaceMode},
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
    use domain::elements::commission::CommissionId;
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

// AC1 — creation itself mints the root surface: mode Total (born Private), the
// creator's envelope, no children. Nothing else can ever remove it: no removal
// route exists on the tree at all (pruning arrives — root-guarded — in ZMVP-73).
#[tokio::test]
async fn a_created_commission_is_born_with_its_root_surface() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;

    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("sign-in provisioned me");

    let tree = backend
        .commission_store()
        .load_tree(domain::elements::commission::CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("creation minted the tree");
    assert!(
        matches!(
            tree.root.kind,
            NodeKind::Surface {
                mode: SurfaceMode::Total
            }
        ),
        "born Private = root Total"
    );
    assert_eq!(
        tree.root.created_by, me.id,
        "the envelope names the creator"
    );
    assert!(tree.root.children.is_empty());
}

// AC2/AC3/AC4 — the owner grows the tree: two surfaces under the root, one
// nested under a non-root surface; every one is born Total with a full
// core-owned envelope; the 201 bodies carry the ids that reappear in the tree.
#[tokio::test]
async fn the_owner_adds_surfaces_under_any_existing_surface() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = create_commission(&client, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let first = add_surface(&client, &base, id, root).await;
    let second = add_surface(&client, &base, id, root).await;
    let nested = add_surface(&client, &base, id, first).await;

    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    let tree = backend
        .commission_store()
        .load_tree(domain::elements::commission::CommissionId::new(id))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 2);
    assert_eq!(*tree.root.children[0].id, first, "append order");
    assert_eq!(*tree.root.children[1].id, second);
    assert_eq!(
        *tree.root.children[0].children[0].id, nested,
        "grows under a non-root surface too"
    );
    for child in &tree.root.children {
        assert!(
            matches!(
                child.kind,
                NodeKind::Surface {
                    mode: SurfaceMode::Total
                }
            ),
            "every new surface is born Total (AC3)"
        );
        assert_eq!(child.created_by, me.id, "the envelope names the creator");
    }

    // Tree edits are NOT changelog events (the taxonomy is frozen; ZMVP-87):
    // the stream still holds only the creation entry.
    let entries = backend
        .changelog_entries(domain::elements::commission::CommissionId::new(id))
        .await
        .expect("changelog");
    assert_eq!(
        entries.len(),
        1,
        "adding surfaces appends no changelog entry"
    );
}

// Floor — anonymous callers can't grow trees: 401, and nothing lands.
#[tokio::test]
async fn an_anonymous_caller_cannot_add_a_surface() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let signed_in = client();
    sign_in(&signed_in, &base).await;
    let id = create_commission(&signed_in, &base, &backend).await;
    let root = root_of(&backend, id).await;

    let res = client()
        .post(format!("{base}/commissions/{id}/surfaces"))
        .json(&json!({ "parent": root }))
        .send()
        .await
        .expect("anonymous POST");
    common::assert_problem(res, 401, "not_authenticated").await;
}

// Floor (the closed door) — a signed-in NON-participant probing someone else's
// commission gets the one uniform commission_not_found 404, byte-identical to
// the answer for a commission that does not exist at all. Never a 403: a 403
// would confirm there is something to be forbidden from.
#[tokio::test]
async fn a_non_participant_gets_the_uniform_not_found() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;
    let foreign = seed_foreign_commission(&backend).await;
    let foreign_root = root_of(&backend, foreign).await;

    // Probing a real commission I may not see...
    let hidden = client
        .post(format!("{base}/commissions/{foreign}/surfaces"))
        .json(&json!({ "parent": foreign_root }))
        .send()
        .await
        .expect("probe foreign");
    let hidden_status = hidden.status().as_u16();
    let hidden_body: serde_json::Value = hidden.json().await.expect("problem body");

    // ...answers exactly like probing one that doesn't exist.
    let absent_id = uuid::Uuid::now_v7();
    let absent = client
        .post(format!("{base}/commissions/{absent_id}/surfaces"))
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
        .load_tree(domain::elements::commission::CommissionId::new(foreign))
        .await
        .expect("load tree")
        .expect("tree exists");
    assert!(tree.root.children.is_empty());
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

    // Fabricated parent id.
    let res = client
        .post(format!("{base}/commissions/{id}/surfaces"))
        .json(&json!({ "parent": uuid::Uuid::now_v7() }))
        .send()
        .await
        .expect("POST fabricated parent");
    common::assert_problem(res, 404, "node_not_found").await;

    // A real node — in someone else's tree.
    let foreign = seed_foreign_commission(&backend).await;
    let foreign_root = root_of(&backend, foreign).await;
    let res = client
        .post(format!("{base}/commissions/{id}/surfaces"))
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
        .post(format!("{base}/commissions/{id}/surfaces"))
        .json(&json!({ "parents": "not-a-parent" }))
        .send()
        .await
        .expect("POST malformed");
    common::assert_problem(res, 422, "invalid_request").await;
}
