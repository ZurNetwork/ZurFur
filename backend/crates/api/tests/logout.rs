//! The exit door (ZMVP-11). Drives the real HTTP stack with every external
//! dependency faked in-process — the PDS (`MemAuthenticator`), the user store
//! (`MemBackend`), and the session store (`MemoryStore`) — so the test is about
//! the sign-out route, not the storage tech (`PgSessionStore` is exercised in
//! adapter-pg's own tests). Asserts both criteria: a signed-out visitor carries no
//! session on the next request, and a second sign-out from a stale tab is harmless.
use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend};
use api::{AppState, Config, Environment};
use domain::elements::{did::Did, profile::Profile};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};

#[tokio::test]
async fn sign_out_destroys_the_session_and_a_second_sign_out_is_harmless() {
    let did = "did:plc:logoutalice";

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let backend = MemBackend::new();
    let state = AppState {
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        files: backend.file_store(),
        database: backend.database(),
        did_minter: Arc::new(adapter_mem::MemDidMinter::new()),
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
        // No route exercised here touches the database, so a lazy (never-connected)
        // pool keeps the test free of a container.
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new(did.to_string()))),
        users: backend.user_store(),
        profile_source: Arc::new(adapter_mem::MemProfileSource::new(Profile {
            did: Did::new(did.to_string()),
            handle: "logoutalice.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: backend.profile_cache(),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Keeps cookies (so the session survives across requests) but does not auto-follow
    // redirects, so each hop can be asserted on its own.
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds");
    let base = format!("http://{addr}");

    // Sign in: start the flow and complete the callback, leaving a live session.
    client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=logoutalice.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");

    // Precondition: the session resolves to the signed-in visitor.
    let res = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    assert_eq!(res.status(), 200, "precondition: visitor is signed in");

    // Sign out: the exit door redirects to the sign-in page.
    let res = client
        .post(format!("{base}/logout"))
        .send()
        .await
        .expect("POST /logout");
    assert_eq!(res.status(), 303, "sign-out redirects");
    assert_eq!(
        res.headers()["location"],
        "/",
        "sign-out lands on the sign-in page"
    );

    // Criterion 1: the next request carries no session — a signed-out user is a
    // visitor again, bounced from the gated route rather than shown a stale identity.
    let res = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me after logout");
    assert_eq!(res.status(), 303, "a signed-out visitor has no session");
    assert_eq!(
        res.headers()["location"],
        "/",
        "and is sent to the sign-in page"
    );

    // Criterion 2: a second sign-out from a stale tab is harmless — the session is
    // already gone, so this lands on the sign-in page, not an error.
    let res = client
        .post(format!("{base}/logout"))
        .send()
        .await
        .expect("POST /logout (stale tab)");
    assert_eq!(res.status(), 303, "a second sign-out is harmless");
    assert_eq!(
        res.headers()["location"],
        "/",
        "and still lands on the sign-in page"
    );
}
