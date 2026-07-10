//! ZMVP-24: signing in rotates the session id (session-fixation hardening).
//!
//! A session id that already exists before the privilege change must not survive
//! it: `Session::cycle_id()` mints a fresh id on a successful sign-in while
//! preserving the session's data. Same in-process fakes as the other sign-in e2e
//! tests — no network, no database.
use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use domain::elements::{did::Did, profile::Profile};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};

/// Boots the app with everything faked in-process and an in-memory session store,
/// returning the base URL. No route exercised here touches the database.
async fn spawn_app(did: &str) -> String {
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
            // ZMVP-49 config (unused by the mem minter in these tests).
            did_key_root_key: "unused-in-tests".to_string(),
            plc_directory_endpoint: "https://plc.directory".to_string(),
            plc_directory_submit: false,
            deadline_sweep_interval_secs: 60,
            max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
        },
        // No route here touches the database, so a lazy (never-connected) pool keeps
        // the test free of a container.
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new(did.to_string()))),
        users: backend.user_store(),
        profile_source: Arc::new(MemProfileSource::new(Profile {
            did: Did::new(did.to_string()),
            handle: "owner.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: backend.profile_cache(),
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        files: backend.file_store(),
        database: backend.database(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

/// A cookie-keeping client that does not auto-follow redirects, so each hop is
/// asserted on its own (same harness as the sign-in e2e).
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds")
}

/// Completes the two-step sign-in and returns the callback response, whose
/// `Set-Cookie` carries the session id.
async fn sign_in(client: &reqwest::Client, base: &str) -> reqwest::Response {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=owner.bsky.social")
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
    res
}

/// The session id this response sets via its `id` cookie (tower-sessions' default
/// cookie name), if any.
fn session_id(res: &reqwest::Response) -> Option<String> {
    res.cookies()
        .find(|c| c.name() == "id")
        .map(|c| c.value().to_string())
}

#[tokio::test]
async fn sign_in_rotates_the_session_id() {
    let base = spawn_app("did:plc:fixation").await;
    let client = client();

    // First sign-in establishes a session id that now exists in the store.
    let first = sign_in(&client, &base).await;
    let id_before = session_id(&first).expect("first sign-in sets a session id");

    // Signing in again carries that established, store-backed id into the privilege
    // change — exactly the id that a fixation attacker would have planted.
    let second = sign_in(&client, &base).await;
    let id_after = session_id(&second).expect("sign-in rotates, so it sets a new session id");

    // AC1: the pre-existing id does not survive the privilege change.
    assert_ne!(
        id_before, id_after,
        "the session id must rotate on sign-in (session-fixation hardening)"
    );

    // AC2: rotation preserves the session — it still resolves to the same User.
    let me = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    assert_eq!(
        me.status(),
        200,
        "the rotated session still resolves to the signed-in User"
    );
}
