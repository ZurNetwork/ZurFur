//! Signing in rotates the session id (session-fixation hardening).
//!
//! A session id that already exists before the privilege change must not survive
//! it: `Session::cycle_id()` mints a fresh id on a successful sign-in while
//! preserving the session's data. Same in-process fakes as the other sign-in e2e
//! tests — no network, no database.
use domain::elements::{did::Did, profile::Profile};
use test_support::http::{client, serve, sign_in};

/// The session id this response sets via its `id` cookie (tower-sessions' default
/// cookie name), if any.
fn session_id(res: &reqwest::Response) -> Option<String> {
    res.cookies()
        .find(|c| c.name() == "id")
        .map(|c| c.value().to_string())
}

#[tokio::test]
async fn sign_in_rotates_the_session_id() {
    let served = serve(
        test_support::runtime::mem(&Did::from("did:plc:fixation".to_string())).profile(
            Profile::new(
                Did::from("did:plc:fixation".to_string()),
                "owner.bsky.social",
            ),
        ),
        api::app,
    )
    .await;
    let base = served.base_url;
    let client = client();

    // First sign-in establishes a session id that now exists in the store.
    let first = sign_in(&client, &base, "owner.bsky.social").await;
    let id_before = session_id(&first).expect("first sign-in sets a session id");

    // Signing in again carries that established, store-backed id into the privilege
    // change — exactly the id that a fixation attacker would have planted.
    let second = sign_in(&client, &base, "owner.bsky.social").await;
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
