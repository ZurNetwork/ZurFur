//! ZMVP-33: an Owner transfers Account ownership to another member
//! (`POST /accounts/{id}/transfer`). Covers the four acceptance criteria — the
//! transfer is immediate and effective, the named member becomes the sole Owner, the
//! prior Owner becomes Admin, and only the current Owner may transfer and only to an
//! existing member — plus the ZMVP-21 enablement (a former Owner, now Admin, can
//! leave). Authority and the "another member" rule are the handler's, so they're
//! exercised here against the in-process fakes; the `parent` re-homing (rule 5) is the
//! store's job and is proven against PostgreSQL in `adapter-pg`'s own tests (the mem
//! fake doesn't model `parent`). DESIGN/Roles rule 8.
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
/// repo handles so a test can seed and introspect membership directly. The signed-in
/// user (via [`sign_in`]) resolves to `did`.
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

/// Drives the two-step sign-in so the client's cookie jar carries a live session for
/// the app's configured DID.
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

/// Founds an account for the signed-in Owner and returns its id.
async fn found_account(client: &reqwest::Client, base: &str, name: &str, handle: &str) -> String {
    let res = client
        .post(format!("{base}/accounts"))
        .json(&json!({ "name": name, "handle": handle }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201, "founding returns 201");
    let body: Value = res.json().await.expect("json");
    body["id"].as_str().expect("account id").to_string()
}

#[tokio::test]
async fn owner_transfers_ownership_and_the_roles_swap() {
    let (base, backend) = spawn_app("did:plc:xferowner").await;
    let client = client();
    sign_in(&client, &base).await;

    let account_id = found_account(&client, &base, "Hand-Off Studio", "handoff.zurfur.app").await;

    // Seat a second, existing member — the transfer target.
    let heir = backend
        .provision(&Did::new("did:plc:heir".to_string()))
        .await
        .expect("provision heir");
    let owner = backend
        .find_by_did(&Did::new("did:plc:xferowner".to_string()))
        .await
        .expect("find owner")
        .expect("sign-in provisioned owner");
    backend
        .grant_role(&UserAccount {
            user_id: heir.id,
            account_id: domain::elements::account::AccountId::new(
                Uuid::parse_str(&account_id).unwrap(),
            ),
            role: Role::Member(None),
        })
        .await
        .expect("seat the heir as a member");

    let res = client
        .post(format!("{base}/accounts/{account_id}/transfer"))
        .json(&json!({ "new_owner": "did:plc:heir" }))
        .send()
        .await
        .expect("POST /transfer");
    assert_eq!(res.status(), 200, "the transfer settles immediately");
    let body: Value = res.json().await.expect("json");
    assert_eq!(body["account"].as_str(), Some(account_id.as_str()));
    assert_eq!(body["owner"].as_str(), Some("did:plc:heir"));
    assert_eq!(body["previous_owner"].as_str(), Some("did:plc:xferowner"));

    let account = domain::elements::account::AccountId::new(Uuid::parse_str(&account_id).unwrap());
    // AC: the named member is now the sole Owner; the prior Owner is now Admin.
    assert_eq!(
        backend
            .role_of(heir.id, account)
            .await
            .expect("role_of heir"),
        Some(Role::Owner(None)),
        "the heir is the new Owner",
    );
    assert_eq!(
        backend
            .role_of(owner.id, account)
            .await
            .expect("role_of owner"),
        Some(Role::Admin(None)),
        "the prior Owner is demoted to Admin",
    );
}

#[tokio::test]
async fn only_the_owner_may_transfer() {
    // The signed-in user is a mere Member of an account someone else owns.
    let (base, backend) = spawn_app("did:plc:notowner").await;
    let client = client();
    sign_in(&client, &base).await;

    let me = backend
        .find_by_did(&Did::new("did:plc:notowner".to_string()))
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
        .post(format!("{base}/accounts/{}/transfer", *account.id))
        .json(&json!({ "new_owner": "did:plc:host" }))
        .send()
        .await
        .expect("POST /transfer");
    common::assert_problem(res, 403, "forbidden").await;
}

#[tokio::test]
async fn cannot_transfer_to_a_non_member() {
    let (base, backend) = spawn_app("did:plc:lonelyowner").await;
    let client = client();
    sign_in(&client, &base).await;

    let account_id = found_account(&client, &base, "Solo Studio", "solo.zurfur.app").await;

    // A user who exists but holds no membership in this account is not a valid target.
    backend
        .provision(&Did::new("did:plc:stranger".to_string()))
        .await
        .expect("provision stranger");

    let res = client
        .post(format!("{base}/accounts/{account_id}/transfer"))
        .json(&json!({ "new_owner": "did:plc:stranger" }))
        .send()
        .await
        .expect("POST /transfer");
    common::assert_problem(res, 404, "member_not_found").await;
}

#[tokio::test]
async fn cannot_transfer_to_an_unknown_did() {
    let (base, _backend) = spawn_app("did:plc:owner2").await;
    let client = client();
    sign_in(&client, &base).await;

    let account_id = found_account(&client, &base, "Studio Two", "two.zurfur.app").await;

    // A DID we have never recognized is, by definition, not a member.
    let res = client
        .post(format!("{base}/accounts/{account_id}/transfer"))
        .json(&json!({ "new_owner": "did:plc:neverseen" }))
        .send()
        .await
        .expect("POST /transfer");
    common::assert_problem(res, 404, "member_not_found").await;
}

#[tokio::test]
async fn cannot_transfer_to_yourself() {
    let (base, _backend) = spawn_app("did:plc:selfxfer").await;
    let client = client();
    sign_in(&client, &base).await;

    let account_id = found_account(&client, &base, "Mine Studio", "mine.zurfur.app").await;

    // Ownership moves to *another* member (Roles rule 8) — self-transfer is refused.
    let res = client
        .post(format!("{base}/accounts/{account_id}/transfer"))
        .json(&json!({ "new_owner": "did:plc:selfxfer" }))
        .send()
        .await
        .expect("POST /transfer");
    common::assert_problem(res, 422, "invalid_request").await;
}

#[tokio::test]
async fn transferring_on_a_missing_account_is_404() {
    let (base, _backend) = spawn_app("did:plc:ghostowner").await;
    let client = client();
    sign_in(&client, &base).await;

    let res = client
        .post(format!("{base}/accounts/{}/transfer", Uuid::now_v7()))
        .json(&json!({ "new_owner": "did:plc:whoever" }))
        .send()
        .await
        .expect("POST /transfer");
    common::assert_problem(res, 404, "account_not_found").await;
}

#[tokio::test]
async fn after_transfer_the_former_owner_can_leave() {
    // The ZMVP-21 enablement: a sole Owner can't leave, but after transferring they
    // are an Admin and may walk out.
    let (base, backend) = spawn_app("did:plc:exitowner").await;
    let client = client();
    sign_in(&client, &base).await;

    let account_id = found_account(&client, &base, "Exit Studio", "exit.zurfur.app").await;
    let account = domain::elements::account::AccountId::new(Uuid::parse_str(&account_id).unwrap());

    let heir = backend
        .provision(&Did::new("did:plc:successor".to_string()))
        .await
        .expect("provision successor");
    backend
        .grant_role(&UserAccount {
            user_id: heir.id,
            account_id: account,
            role: Role::Member(None),
        })
        .await
        .expect("seat the successor");

    // Before transfer, the Owner cannot leave.
    let res = client
        .delete(format!("{base}/accounts/{account_id}/members/me"))
        .send()
        .await
        .expect("DELETE members/me");
    common::assert_problem(res, 409, "owner_cannot_leave").await;

    // Transfer, then the former Owner (now Admin) may leave.
    let res = client
        .post(format!("{base}/accounts/{account_id}/transfer"))
        .json(&json!({ "new_owner": "did:plc:successor" }))
        .send()
        .await
        .expect("POST /transfer");
    assert_eq!(res.status(), 200, "transfer settles");

    let res = client
        .delete(format!("{base}/accounts/{account_id}/members/me"))
        .send()
        .await
        .expect("DELETE members/me");
    assert_eq!(res.status(), 204, "a former Owner, now Admin, may leave");
}
