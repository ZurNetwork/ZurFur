//! ZMVP-12 criterion 1: a signed-in user is still signed in after a server
//! restart. Persistence is the whole point, so this test wires the session layer
//! to the durable `PgSessionStore` (not `MemoryStore`) over a real PostgreSQL
//! container — the session row must outlive the process. The PDS and the user
//! store are still faked in-process (`MemAuthenticator`, `MemBackend`), so the
//! test stays about session durability, not OAuth or the user repo.
//!
//! "Restart" is simulated by dropping the first app/router/store and building a
//! brand-new app + a brand-new `PgSessionStore` over the *same* database pool:
//! nothing in-memory survives, only the Postgres rows. A request to `/me` carrying
//! the cookie minted before the "restart" must still resolve to the signed-in
//! visitor (200), not bounce to the sign-in page (303). Requires a container
//! runtime socket.
use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend};
use adapter_pg::{PgPool, PgSessionStore};
use api::{AppState, Config, Environment};
use domain::elements::{did::Did, profile::Profile};
use reqwest::redirect::Policy;
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};
use tower_sessions::SessionManagerLayer;

/// Builds the app router wired to a fresh `PgSessionStore` over `pool`, serves it
/// on an ephemeral port, and returns the base URL. The `backend` is shared so a
/// "restarted" instance resolves the same User the cookie points at — what we are
/// proving durable is the *session*, kept in Postgres, not the repo.
async fn serve(pool: PgPool, did: &str, backend: MemBackend) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");
    let state = AppState {
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        files: backend.file_store(),
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
        pool: pool.clone(),
        auth: Arc::new(MemAuthenticator::new(Did::new(did.to_string()))),
        users: backend.user_store(),
        profile_source: Arc::new(adapter_mem::MemProfileSource::new(Profile {
            did: Did::new(did.to_string()),
            handle: "persistalice.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: backend.profile_cache(),
        database: backend.database(),
    };
    // The session layer backs the cookie with Postgres, so the row survives the
    // "restart" simulated below by tearing down this app and building another.
    let app = api::app(state).layer(SessionManagerLayer::new(PgSessionStore::new(pool)));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn a_signed_in_user_is_still_signed_in_after_a_server_restart() {
    let did = "did:plc:persistalice";

    // Spin up Postgres and run migrations (mirrors adapter-pg's session_store tests).
    let container = Postgres::default()
        .start()
        .await
        .expect("postgres container should start");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped postgres port");
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = adapter_pg::connect(&database_url)
        .await
        .expect("pool connects");
    adapter_pg::migrate(&pool).await.expect("migrations run");

    // The user the cookie will point at. Shared across both app instances so the
    // "restart" resolves the same identity — the durable part under test is the
    // session row, which lives in Postgres.
    let backend = MemBackend::new();
    backend
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision seeds the recognized user");

    // --- First boot: sign in, leaving a real session row in Postgres. ---
    let base = serve(pool.clone(), did, backend.clone()).await;
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds");

    client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=persistalice.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    let callback = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");

    // Capture the session cookie the server minted. Replaying it by hand against a
    // fresh client (no shared cookie jar) is exactly a browser hitting a restarted
    // server: only what Postgres persisted can carry the session across.
    let set_cookie = callback
        .headers()
        .get("set-cookie")
        .expect("sign-in mints a session cookie")
        .to_str()
        .expect("cookie header is valid text")
        .to_string();
    let cookie = set_cookie
        .split(';')
        .next()
        .expect("cookie name=value pair")
        .to_string();

    // Precondition: on the original instance, the session resolves to the signed-in
    // visitor.
    let res = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me before restart");
    assert_eq!(res.status(), 200, "precondition: visitor is signed in");

    // --- Simulate the restart. ---
    // Drop the first client and let the first app instance go: nothing in-process
    // carries over. Only the Postgres `tower_sessions.session` row remains.
    drop(client);

    // Build a brand-new app + a brand-new PgSessionStore over the SAME pool/database.
    let restarted_base = serve(pool.clone(), did, backend.clone()).await;

    // A fresh client with no cookie jar: the only thing tying it to the prior
    // session is the cookie we captured — and the row that cookie keys, in Postgres.
    let fresh_client = reqwest::Client::builder()
        .redirect(Policy::none())
        .build()
        .expect("fresh client builds");

    let res = fresh_client
        .get(format!("{restarted_base}/me"))
        .header("cookie", &cookie)
        .send()
        .await
        .expect("GET /me after restart");

    // Criterion 1: the session survived the restart — the brand-new server, reading
    // the persisted row through a brand-new PgSessionStore, still recognizes the
    // visitor. A 303 to "/" here would mean the session was lost (e.g. a MemoryStore
    // would have nothing after restart); 200 with the handle proves durability.
    assert_eq!(
        res.status(),
        200,
        "the session must survive a server restart"
    );
    let body = res.text().await.expect("read body");
    assert!(
        body.contains("persistalice.bsky.social"),
        "the restored page greets the still-signed-in visitor, got: {body}"
    );
}
