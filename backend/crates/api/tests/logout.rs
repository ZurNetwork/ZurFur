//! The exit door (ZMVP-11). Drives the real HTTP stack with every external
//! dependency faked in-process — the PDS (`MemAuthenticator`), the user store
//! (`MemBackend`), and the session store (`MemoryStore`) — so the test is about
//! the sign-out route, not the storage tech (`PgSessionStore` is exercised in
//! adapter-pg's own tests). Asserts both criteria: a signed-out visitor carries no
//! session on the next request, and a second sign-out from a stale tab is harmless.
use api::AppState;
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

    let test_support::runtime::MemRuntime {
        runtime,
        backend: _,
    } = test_support::runtime::mem(&Did::new(did.to_string()))
        .profile(Profile::new(
            Did::new(did.to_string()),
            "logoutalice.bsky.social",
        ))
        .public_url(format!("http://{addr}"))
        .build();
    let state: AppState = runtime;
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
    // visitor again, refused the gated read (401) rather than shown a stale identity.
    let res = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me after logout");
    assert_eq!(
        res.status(),
        401,
        "a signed-out visitor has no session, so /me is unauthenticated"
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
