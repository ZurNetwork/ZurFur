//! The PostgreSQL test setup, proven against the real app: sign-in goes through
//! the in-process sign-in, and every row the app writes — the User, the session,
//! an Account — lands in the test's private PostgreSQL database. Requires a
//! container runtime socket.
use domain::elements::{did::Did, profile::Profile};
use test_support::pg_app;

mod common;

/// The PDS identity the setup signs alice in as.
fn alice() -> Profile {
    Profile::new(Did::from("did:plc:pgsetupalice".to_string()), "alice.test")
}

/// The PDS identity the setup signs bob in as.
fn bob() -> Profile {
    Profile::new(Did::from("did:plc:pgsetupbob".to_string()), "bob.test")
}

/// How many `users` rows carry `did`, read straight from PostgreSQL.
async fn user_rows(pool: &adapter_pg::PgPool, did: &Did) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM users WHERE id = $1")
        .bind(did.as_ref())
        .fetch_one(pool)
        .await
        .expect("count users rows")
}

/// How many PostgreSQL session rows hold `did`'s sign-in. The session data is
/// MessagePack, which stores the DID's bytes as they are.
async fn session_rows(pool: &adapter_pg::PgPool, did: &Did) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM tower_sessions.session WHERE position($1::bytea IN data) > 0",
    )
    .bind(did.as_ref().as_bytes())
    .fetch_one(pool)
    .await
    .expect("count session rows")
}

#[tokio::test]
async fn signing_in_provisions_the_user_in_postgres() {
    let served = pg_app::serve(api::app).await;
    let alice = alice();
    assert_eq!(
        user_rows(&served.runtime.pool, &alice.did).await,
        0,
        "precondition: no User before the first sign-in"
    );

    served.sign_in(alice.clone()).await;

    assert_eq!(
        user_rows(&served.runtime.pool, &alice.did).await,
        1,
        "the sign-in callback provisions the User in PostgreSQL"
    );
}

#[tokio::test]
async fn each_person_is_signed_in_as_themselves() {
    let served = pg_app::serve(api::app).await;
    let alice_client = served.sign_in(alice()).await;
    let bob_client = served.sign_in(bob()).await;

    let alice_me: serde_json::Value = alice_client
        .get(format!("{}/me", served.base_url))
        .send()
        .await
        .expect("GET /me as alice")
        .json()
        .await
        .expect("alice's /me is JSON");
    let bob_me: serde_json::Value = bob_client
        .get(format!("{}/me", served.base_url))
        .send()
        .await
        .expect("GET /me as bob")
        .json()
        .await
        .expect("bob's /me is JSON");

    let expected_alice =
        serde_json::json!({ "did": "did:plc:pgsetupalice", "handle": "alice.test" });
    let expected_bob = serde_json::json!({ "did": "did:plc:pgsetupbob", "handle": "bob.test" });
    assert_eq!(alice_me, expected_alice, "alice's session is alice's");
    assert_eq!(bob_me, expected_bob, "bob's session is bob's");
}

#[tokio::test]
async fn the_sessions_live_in_postgres() {
    let served = pg_app::serve(api::app).await;
    served.sign_in(alice()).await;
    served.sign_in(bob()).await;

    let alice_sessions = session_rows(&served.runtime.pool, &alice().did).await;
    let bob_sessions = session_rows(&served.runtime.pool, &bob().did).await;

    assert_eq!(alice_sessions, 1, "alice's sign-in wrote one session row");
    assert_eq!(bob_sessions, 1, "bob's sign-in wrote one session row");
}

#[tokio::test]
async fn a_handle_the_setup_never_signed_in_is_refused() {
    let served = pg_app::serve(api::app).await;
    let stranger = test_support::http::client();

    let res = stranger
        .post(format!("{}/signin", served.base_url))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=stranger.test")
        .send()
        .await
        .expect("POST /signin");

    common::assert_problem(res, 422, "invalid_request").await;
}

#[tokio::test]
async fn an_account_founded_through_the_app_lands_in_postgres() {
    let served = pg_app::serve(api::app).await;
    let alice_client = served.sign_in(alice()).await;

    let founding =
        serde_json::json!({ "name": "Alice Studio", "handle": "alicestudio.zurfur.app" });
    let res = alice_client
        .post(format!("{}/accounts", served.base_url))
        .json(&founding)
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201, "founding an account returns 201 Created");
    let body: serde_json::Value = res.json().await.expect("json body");
    let account_did = body["did"].as_str().expect("the account's DID");

    let accounts: i64 = sqlx::query_scalar("SELECT count(*) FROM accounts WHERE id = $1")
        .bind(account_did)
        .fetch_one(&served.runtime.pool)
        .await
        .expect("count accounts rows");
    assert_eq!(accounts, 1, "the founded Account is a PostgreSQL row");
}
