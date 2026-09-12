//! End-to-end sign-in flow with every external dependency faked in-process: the
//! PDS (a `MemAuthenticator` that authenticates as a fixed DID), the private store
//! (`MemBackend`), and the session store (`MemoryStore`). The only seam to the
//! outside world — the OAuth handshake with the PDS — is the `Authenticator` port,
//! so this drives the real HTTP stack, routing, session layer, provisioning, and
//! session→User resolution without a network or a database.
use api::AppState;
use domain::elements::{did::Did, profile::Profile};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};

#[tokio::test]
async fn first_sign_in_provisions_a_user_and_the_session_resolves_to_it() {
    let did = "did:plc:e2ealice";

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    // Hold a handle to the shared backend so we can introspect it after the flow.
    let test_support::runtime::MemRuntime { runtime, backend } =
        test_support::runtime::mem(&Did::new(did.to_string()))
            .profile(Profile::new(
                Did::new(did.to_string()),
                "e2ealice.bsky.social",
            ))
            .public_url(format!("http://{addr}"))
            .build();
    let state: AppState = runtime;
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    // Keeps cookies (so the session survives across requests) but does not auto-follow
    // redirects, so each hop of the flow can be asserted on its own.
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client builds");
    let base = format!("http://{addr}");

    // 1. Start sign-in: the handler redirects to the PDS authorization URL.
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=alice.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(res.status(), 303, "signin should redirect to the PDS");

    // 2. The PDS redirects back: the callback provisions a User and stores its id.
    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303, "callback should redirect on success");
    assert_eq!(
        res.headers()["location"],
        "/",
        "callback lands the visitor on the signed-in frontend root"
    );

    // 3. A subsequent request resolves the session back to the provisioned User —
    //    identified by their profile handle in the JSON whoami, with no PDS
    //    round-trip for the identity itself. (Profile rendering is covered in
    //    profile.rs.)
    let res = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    assert_eq!(res.status(), 200, "the signed-in visitor is recognized");
    let body: serde_json::Value = res.json().await.expect("body is JSON");
    assert_eq!(
        body["handle"], "e2ealice.bsky.social",
        "/me identifies the signed-in visitor, got: {body}"
    );

    // Exactly one User exists for that DID after a successful sign-in.
    let provisioned = backend
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision");

    // 4. A repeat sign-in finds the SAME User — one DID, one User, forever. A second
    //    callback re-enters provisioning; it must return the existing User, not mint
    //    a new one, so the id is unchanged.
    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback (repeat)");
    assert_eq!(res.status(), 303);

    let again = backend
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision (repeat)");
    assert_eq!(
        provisioned.id, again.id,
        "a repeat sign-in must reuse the same User, never duplicate"
    );
    assert_eq!(
        provisioned.created_at, again.created_at,
        "recognition time is stamped once, on first contact"
    );
}
