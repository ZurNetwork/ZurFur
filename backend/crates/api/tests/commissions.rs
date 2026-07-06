//! ZMVP-65 — a User creates a commission and owns it, end to end over HTTP.
//!
//! Pins the acceptance criteria at the API surface (the mem store-layer tests in
//! `adapter-mem` cover the persistence seam):
//!
//! - **AC1** — a signed-in User creates a commission by supplying a Title → `201`;
//! - **AC2/AC3** — the creating User is the owner and the commission is born in
//!   `Draft` with `Private` visibility (introspected off the backend, since the route
//!   returns a bare `201`);
//! - **AC4** — a User with **no Account** can create one (a user-scoped write; not
//!   gated on account membership — ZMVP-47, DD 26247170 §5);
//! - and the floors: an **anonymous** caller cannot create a commission (`401`), and
//!   a **blank title** is rejected (`422`, the `CommissionTitle` gate).
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use domain::elements::{
    commission::{LifecycleStep, Visibility},
    did::Did,
    profile::Profile,
};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] so a test can introspect the commissions that were persisted.
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

// AC1/AC2/AC3 — a signed-in User creates a commission by Title, becomes its owner,
// and it is born in `Draft`. The route returns a bare `201`, so owner + lifecycle
// are read back off the shared backend.
#[tokio::test]
async fn signed_in_user_creates_a_commission_and_owns_it() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;

    let me = backend
        .find_by_did(&Did::new("did:plc:artist".to_string()))
        .await
        .expect("find me")
        .expect("sign-in provisioned me");

    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "A ref sheet" }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(res.status(), 201, "creating a commission returns 201");

    let all = backend.all_commissions().await.expect("list commissions");
    assert_eq!(all.len(), 1, "exactly one commission was persisted");
    let commission = &all[0];
    assert_eq!(
        commission.title.as_str(),
        "A ref sheet",
        "the Title round-trips"
    );
    assert_eq!(commission.owner_id, me.id, "the creating User is the owner");
    assert!(
        matches!(commission.lifecycle_step, LifecycleStep::Draft),
        "a new commission is born in Draft",
    );
    assert!(
        matches!(commission.visibility, Visibility::Private),
        "a new commission is born Private (AC3)",
    );
}

// AC4 — a signed-in User holding ZERO accounts can still create a commission:
// creating one is a user-scoped write, not gated on account membership (ZMVP-47).
#[tokio::test]
async fn a_user_with_no_account_can_create_a_commission() {
    let (base, backend) = spawn_app("did:plc:newcomer").await;
    let client = client();
    sign_in(&client, &base).await;
    // The signed-in user founds no account first — they hold none.

    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "First commission" }))
        .send()
        .await
        .expect("POST /commissions");
    assert_eq!(
        res.status(),
        201,
        "a zero-account User can create a commission (user-scoped write)",
    );
    assert_eq!(
        backend.all_commissions().await.expect("list").len(),
        1,
        "the commission was persisted",
    );
}

// The floor — an anonymous (signed-out) caller cannot create a commission: turned
// away at 401 `not_authenticated` (problem+json), and nothing is persisted.
#[tokio::test]
async fn anonymous_cannot_create_a_commission() {
    let (base, backend) = spawn_app("did:plc:nobody").await;

    // No sign-in: the cookie jar carries no session.
    let res = client()
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "Should not persist" }))
        .send()
        .await
        .expect("POST /commissions");
    common::assert_problem(res, 401, "not_authenticated").await;

    assert!(
        backend.all_commissions().await.expect("list").is_empty(),
        "an unauthenticated create persists nothing",
    );
}

// Title validation — a blank (whitespace-only) title is rejected as `422`
// `invalid_request` (the `CommissionTitle` gate), and nothing is persisted.
#[tokio::test]
async fn a_blank_title_is_rejected() {
    let (base, backend) = spawn_app("did:plc:artist").await;
    let client = client();
    sign_in(&client, &base).await;

    let res = client
        .post(format!("{base}/commissions"))
        .json(&json!({ "title": "   " }))
        .send()
        .await
        .expect("POST /commissions");
    common::assert_problem(res, 422, "invalid_request").await;

    assert!(
        backend.all_commissions().await.expect("list").is_empty(),
        "a blank-title create persists nothing",
    );
}
