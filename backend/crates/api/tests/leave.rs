//! ZMVP-21: a member leaves their own account (`DELETE /accounts/{id}/members/me`).
//! Covers the handler-side preconditions (Owner can't leave → 409, a non-member →
//! 404) and the happy path (a member leaves → 204, and is no longer a member). The
//! role-tree re-homing and invitation revocation are the store's job and are proven
//! against PostgreSQL in `adapter-pg`'s own tests (the mem fake doesn't model
//! `parent`). Same in-process fakes as the other account e2e tests.
use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    account::{Account, AccountName},
    did::Did,
    handle::Handle,
    profile::Profile,
    role::Role,
    user_account::UserAccount,
};
use reqwest::redirect::Policy;
use serde_json::{Value, json};
use tower_sessions::{MemoryStore, SessionManagerLayer};
use uuid::Uuid;

mod common;

/// Boots the app with everything faked in-process; returns the base URL plus the
/// repo handles so a test can seed and introspect membership directly.
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
            // ZMVP-49 config (unused by the mem minter in these tests).
            did_key_root_key: "unused-in-tests".to_string(),
            plc_directory_endpoint: "https://plc.directory".to_string(),
            plc_directory_submit: false,
        },
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
        database: backend.database(),
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
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

/// Drives the two-step sign-in so the client's cookie jar carries a live session.
async fn sign_in(client: &reqwest::Client, base: &str) {
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
}

#[tokio::test]
async fn the_owner_cannot_leave_their_own_account() {
    let (base, _backend) = spawn_app("did:plc:leaveowner").await;
    let client = client();
    sign_in(&client, &base).await;

    // Founding makes the signed-in user the Owner.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&json!({ "name": "Solo Studio", "handle": "solo.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201, "founding returns 201");
    let body: Value = res.json().await.expect("json");
    let account_id = body["id"].as_str().expect("account id");

    let res = client
        .delete(format!("{base}/accounts/{account_id}/members/me"))
        .send()
        .await
        .expect("DELETE members/me");
    common::assert_problem(res, 409, "owner_cannot_leave").await;
}

#[tokio::test]
async fn leaving_an_account_you_are_not_a_member_of_is_404() {
    let (base, _backend) = spawn_app("did:plc:leavestranger").await;
    let client = client();
    sign_in(&client, &base).await;

    let res = client
        .delete(format!("{base}/accounts/{}/members/me", Uuid::now_v7()))
        .send()
        .await
        .expect("DELETE members/me");
    common::assert_problem(res, 404, "member_not_found").await;
}

#[tokio::test]
async fn a_member_leaves_and_is_no_longer_a_member() {
    let (base, backend) = spawn_app("did:plc:leaver").await;
    let client = client();
    sign_in(&client, &base).await;

    // The signed-in user is provisioned by sign-in; seat them as a *Member* of an
    // account someone else owns, so leaving isn't blocked by the Owner rule.
    let me = backend
        .find_by_did(&Did::new("did:plc:leaver".to_string()))
        .await
        .expect("find me")
        .expect("sign-in provisioned me");
    let host = backend
        .provision(&Did::new("did:plc:host".to_string()))
        .await
        .expect("provision host");
    let (account, owner_membership) = Account::open(
        host.id,
        Did::new("did:plc:hostacct".to_string()),
        Handle::try_new("host.zurfur.app").unwrap(),
        AccountName::try_new("Host Studio").unwrap(),
        Utc::now(),
    );
    backend
        .create(&account, &owner_membership)
        .await
        .expect("found host account");
    backend
        .grant_role(&UserAccount {
            user_id: me.id,
            account_id: account.id,
            role: Role::Member(None),
        })
        .await
        .expect("seat me as a member");

    let res = client
        .delete(format!("{base}/accounts/{}/members/me", *account.id))
        .send()
        .await
        .expect("DELETE members/me");
    assert_eq!(
        res.status(),
        204,
        "a member leaves on their own action, no approval"
    );

    let role = backend.role_of(me.id, account.id).await.expect("role_of");
    assert_eq!(role, None, "after leaving, the user holds no role");
}
