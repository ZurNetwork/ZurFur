//! `GET /commissions` (ZMVP-157) — the signed-in user's OWNED commissions,
//! owner-POV only:
//!
//! - the listing is owner-scoped (a commission owned by someone else never
//!   appears, even if the caller could otherwise reach it);
//! - archived commissions are excluded (an active-view listing);
//! - ordering is deterministic (by id — UUIDv7 sorts as creation order);
//! - an anonymous caller gets a `401` problem+json, same as `GET /me`.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionTitle},
    did::Did,
    profile::Profile,
};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;
use common::assert_problem;

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] so a test can seed commissions and introspect them. `did` is
/// the identity `sign_in` authenticates as.
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
            handle: "lister.bsky.social".to_string(),
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
        .body("handle=lister.bsky.social")
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

// Owner-scoped listing: a commission owned by a DIFFERENT user never appears,
// even though it exists in the same backend — the non-participant projection
// view (ZMVP-75) is a distinct, later surface this endpoint does not attempt.
// Also pins ordering: ascending by id (UUIDv7 sorts as creation order).
#[tokio::test]
async fn lists_only_commissions_the_caller_owns_in_deterministic_order() {
    let did = "did:plc:owner-lister";
    let (base, backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let me = backend
        .find_by_did(&Did::new(did.to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    // Two commissions owned by the caller, created through the real write path.
    let mine_a = Commission::create(
        CommissionTitle::try_from("First".to_string()).expect("valid title"),
        me.id,
        Utc::now(),
        None,
    );
    backend
        .create_commission(&mine_a)
        .await
        .expect("seed commission A");
    let mine_b = Commission::create(
        CommissionTitle::try_from("Second".to_string()).expect("valid title"),
        me.id,
        Utc::now(),
        None,
    );
    backend
        .create_commission(&mine_b)
        .await
        .expect("seed commission B");

    // A commission owned by SOMEONE ELSE — must never appear on this list.
    let someone_else = backend
        .provision(&Did::new("did:plc:other-owner".to_string()))
        .await
        .expect("provision someone else");
    let theirs = Commission::create(
        CommissionTitle::try_from("Not mine".to_string()).expect("valid title"),
        someone_else.id,
        Utc::now(),
        None,
    );
    backend
        .create_commission(&theirs)
        .await
        .expect("seed someone else's commission");

    let res = client
        .get(format!("{base}/commissions"))
        .send()
        .await
        .expect("GET /commissions");
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.expect("json body");
    let rows = body.as_array().expect("array body");
    assert_eq!(rows.len(), 2, "only the caller's own commissions: {body}");

    let ids: Vec<String> = rows
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    assert!(
        !ids.contains(&theirs.id.to_string()),
        "someone else's commission must never appear"
    );

    let mut expected_order = vec![mine_a.id.to_string(), mine_b.id.to_string()];
    expected_order.sort();
    assert_eq!(
        ids, expected_order,
        "rows are ordered ascending by id (UUIDv7 = creation order)"
    );

    let first_row = rows
        .iter()
        .find(|row| row["id"] == mine_a.id.to_string())
        .expect("commission A is listed");
    assert_eq!(first_row["title"], "First");
    assert_eq!(first_row["lifecycle"], "draft");
    assert_eq!(first_row["visibility"], "private");
}

// Archived commissions are excluded — an active-view listing, per the
// documented listing-projection contract on `Commission::archived_at`
// (Deletion DD 3014657; ZMVP-68).
#[tokio::test]
async fn excludes_archived_commissions() {
    let did = "did:plc:archive-lister";
    let (base, backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let me = backend
        .find_by_did(&Did::new(did.to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    let active = Commission::create(
        CommissionTitle::try_from("Active".to_string()).expect("valid title"),
        me.id,
        Utc::now(),
        None,
    );
    backend
        .create_commission(&active)
        .await
        .expect("seed active commission");

    let mut archived = Commission::create(
        CommissionTitle::try_from("Archived".to_string()).expect("valid title"),
        me.id,
        Utc::now(),
        None,
    );
    archived.archived_at = Some(Utc::now());
    backend
        .create_commission(&archived)
        .await
        .expect("seed archived commission");

    let res = client
        .get(format!("{base}/commissions"))
        .send()
        .await
        .expect("GET /commissions");
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.expect("json body");
    let rows = body.as_array().expect("array body");
    assert_eq!(rows.len(), 1, "the archived commission is excluded: {body}");
    assert_eq!(rows[0]["id"], active.id.to_string());
}

// An anonymous caller gets a 401 problem+json, exactly like `GET /me` — never a
// redirect, since the frontend calls this endpoint.
#[tokio::test]
async fn anonymous_caller_is_turned_away_with_401() {
    let (base, _backend) = spawn_app("did:plc:anon-lister").await;

    let res = client()
        .get(format!("{base}/commissions"))
        .send()
        .await
        .expect("GET /commissions");
    assert_problem(res, 401, "not_authenticated").await;
}
