//! ZMVP-23: defense-in-depth CSRF on the cookie surface (DD "Auth Surfaces, the
//! Plugin Trust Boundary & CSRF"). A state-changing request whose `Origin` header
//! is present and is **not** our first-party origin is rejected (403); a matching
//! origin, a missing origin (a non-browser client, which carries no ambient cookie
//! and so can't be CSRF'd), and safe methods all pass. Layers on top of the session
//! cookie's `SameSite=Lax`. Same in-process fakes as the other api e2e tests — no
//! database (the guard runs before the auth-gated handlers, which 401 first).
use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use domain::elements::{did::Did, profile::Profile};
use reqwest::redirect::Policy;
use serde_json::json;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;

/// Boots the app with everything faked in-process; `public_url` is the app's own
/// address, so it doubles as the one allowed first-party origin.
async fn spawn_app() -> String {
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
        },
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new("did:plc:test".to_string()))),
        users: backend.user_store(),
        profile_source: Arc::new(MemProfileSource::new(Profile {
            did: Did::new("did:plc:test".to_string()),
            handle: "t.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: backend.profile_cache(),
        accounts: backend.account_store(),
        database: backend.database(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .expect("client builds")
}

#[tokio::test]
async fn a_cross_origin_state_changing_request_is_blocked() {
    let base = spawn_app().await;
    let res = client()
        .post(format!("{base}/accounts"))
        .header("origin", "http://evil.example")
        .json(&json!({ "name": "x" }))
        .send()
        .await
        .expect("POST /accounts");
    // Blocked by the guard before it ever reaches the handler.
    common::assert_problem(res, 403, "cross_origin").await;
}

#[tokio::test]
async fn a_same_origin_state_changing_request_passes_the_guard() {
    let base = spawn_app().await;
    // Origin == our first-party origin (`public_url`): the guard passes, and the
    // handler then 401s the unauthenticated request — proving it got through.
    let res = client()
        .post(format!("{base}/accounts"))
        .header("origin", &base)
        .json(&json!({ "name": "x" }))
        .send()
        .await
        .expect("POST /accounts");
    common::assert_problem(res, 401, "not_authenticated").await;
}

#[tokio::test]
async fn a_request_with_no_origin_passes_the_guard() {
    let base = spawn_app().await;
    // No Origin (a non-browser client carrying no ambient cookie) — can't be
    // CSRF'd, so the guard lets it through to the handler's 401.
    let res = client()
        .post(format!("{base}/accounts"))
        .json(&json!({ "name": "x" }))
        .send()
        .await
        .expect("POST /accounts");
    common::assert_problem(res, 401, "not_authenticated").await;
}

#[tokio::test]
async fn a_safe_method_is_never_blocked_by_origin() {
    let base = spawn_app().await;
    // GET is safe; even a foreign Origin passes (anonymous /me redirects to /).
    let res = client()
        .get(format!("{base}/me"))
        .header("origin", "http://evil.example")
        .send()
        .await
        .expect("GET /me");
    assert_eq!(
        res.status(),
        303,
        "a safe method is not subject to the Origin guard"
    );
}
