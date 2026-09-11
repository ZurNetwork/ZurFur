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
use adapter_mem::MemBackend;
use adapter_pg::{PgPool, PgSessionStore};
use api::AppState;
use domain::elements::{did::Did, profile::Profile};
use reqwest::redirect::Policy;
use tower_sessions::SessionManagerLayer;

/// Builds the app router wired to a fresh `PgSessionStore` over `pool`, serves it
/// on an ephemeral port, and returns the base URL. The `backend` is shared so a
/// "restarted" instance resolves the same User the cookie points at — what we are
/// proving durable is the *session*, kept in Postgres, not the repo. The fixture
/// builds its own fresh `MemBackend`/lazy pool per `build()`, so this overrides
/// the store fields (and `pool`) with the shared ones after building — the parts
/// that must survive the simulated restart.
async fn serve(pool: PgPool, did: &str, backend: MemBackend) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let profile = Profile::new(Did::new(did.to_string()), "persistalice.bsky.social");
    let test_support::runtime::MemRuntime { mut runtime, .. } =
        test_support::runtime::mem(&Did::new(did.to_string()))
            .profile(profile)
            .public_url(format!("http://{addr}"))
            .build();
    runtime.pool = pool.clone();
    runtime.accounts = backend.account_store();
    runtime.commissions = backend.commission_store();
    runtime.changelog = backend.changelog_store();
    runtime.files = backend.file_store();
    runtime.users = backend.user_store();
    runtime.profile_cache = backend.profile_cache();
    runtime.database = backend.database();
    let state: AppState = runtime;
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

    // A migrated clone of the shared template database (see `test_support::pg`);
    // `_db` keeps the shared container alive across both app instances.
    let (pool, _db) = test_support::pg::fresh_pool().await;

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
