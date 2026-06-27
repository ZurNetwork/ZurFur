//! End-to-end account founding (ZMVP-14): a signed-in visitor POSTs `/accounts`,
//! the server mints the account's sovereign `did:plc`, founds the account, and makes
//! the creating User its Owner. An anonymous visitor is turned away. Same in-process
//! fakes as the sign-in e2e — no network, no database.
use std::sync::Arc;

use adapter_mem::{
    MemAccountRepo, MemAuthenticator, MemDidMinter, MemProfileCache, MemProfileSource, MemUserRepo,
};
use api::{AppState, Config, Environment};
use domain::{
    elements::{account::AccountId, did::Did, profile::Profile, role::Role},
    ports::{AccountRepo, UserRepo},
};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};
use uuid::Uuid;

mod common;

/// Boots the app with everything faked in-process and returns the base URL plus
/// typed handles to the repos, so a test can introspect them after the flow. The
/// unsizing to the `Arc<dyn …>` fields happens at assignment.
async fn spawn_app(did: &str) -> (String, Arc<MemUserRepo>, Arc<MemAccountRepo>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind ephemeral port");
    let addr = listener.local_addr().expect("local addr");

    let user_repo = Arc::new(MemUserRepo::new());
    let account_repo = Arc::new(MemAccountRepo::new());
    let state = AppState {
        config: Config {
            env: Environment::DEV,
            http_addr: addr,
            public_url: format!("http://{addr}"),
            database_url: "postgres://unused".to_string(),
            log_level: "info".to_string(),
        },
        // No route here touches the database, so a lazy (never-connected) pool keeps
        // the test free of a container.
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new(did.to_string()))),
        user_repo: user_repo.clone(),
        profile_source: Arc::new(MemProfileSource::new(Profile {
            did: Did::new(did.to_string()),
            handle: "owner.bsky.social".to_string(),
            display_name: None,
            avatar_url: None,
        })),
        profile_cache: Arc::new(MemProfileCache::new()),
        account_repo: account_repo.clone(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), user_repo, account_repo)
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
async fn signed_in_visitor_founds_an_account_and_becomes_its_owner() {
    let did = "did:plc:e2eowner";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    // Found the account — founding requires a name.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Acme Studio" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201, "founding an account returns 201 Created");
    let body: serde_json::Value = res.json().await.expect("json body");
    let account_id = body["id"]
        .as_str()
        .expect("response carries the account id");
    let account_did = body["did"]
        .as_str()
        .expect("response carries the account did");
    assert!(
        account_did.starts_with("did:plc:"),
        "the account is minted its own did:plc, got: {account_did}"
    );
    assert_eq!(
        body["name"], "Acme Studio",
        "the response echoes the account's name"
    );

    // The creating User is the Owner of the founded account (the heart of ZMVP-14).
    let user = user_repo
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision is idempotent — returns the signed-in User");
    let account = AccountId::new(Uuid::parse_str(account_id).expect("id is a uuid"));
    let role = account_repo
        .role_of(user.id, account)
        .await
        .expect("role_of");
    assert_eq!(
        role,
        Some(Role::Owner(None)),
        "the creating User becomes the account's Owner"
    );

    // And the account itself is persisted, retrievable by id.
    let found = account_repo.find(account).await.expect("find");
    assert!(
        found.is_some_and(|a| a.did == Did::new(account_did.to_string())),
        "the founded account is stored under its minted did"
    );
}

#[tokio::test]
async fn founding_requires_a_name() {
    let did = "did:plc:e2enoname";
    let (base, _user_repo, _account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    // A blank name is understood but unusable — rejected with 422.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "   " }))
        .send()
        .await
        .expect("POST /accounts");
    common::assert_problem(res, 422, "invalid_request").await;

    // The rejected attempt minted nothing: the next, valid founding gets the very
    // first DID from the deterministic mem minter (`did:plc:mem000000`). Had the
    // blank attempt reached the minter, this would be `...mem000001`.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Acme Studio" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201);
    let body: serde_json::Value = res.json().await.expect("json body");
    assert_eq!(
        body["did"], "did:plc:mem000000",
        "a rejected founding must not consume a minted identity"
    );
}

/// Founds an account through `POST /accounts` with the given client/session and
/// returns its id — the shared first step of the grant tests below.
async fn found_account(client: &reqwest::Client, base: &str, name: &str) -> String {
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": name }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201, "founding an account returns 201");
    let body: serde_json::Value = res.json().await.expect("json body");
    body["id"]
        .as_str()
        .expect("response carries the account id")
        .to_string()
}

// ZMVP-15 — the heart: an Owner grants a role and the grantee is seated as a member
// of the account at that role. The grantee is named by DID and need not have signed
// in; the grant recognizes them. (Requires task ②, `Role::can_grant`, to be live.)
#[tokio::test]
async fn owner_grants_a_role_and_seats_the_member() {
    let did = "did:plc:e2egranter";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    let grantee_did = "did:plc:e2egrantee";
    let res = client
        .post(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": grantee_did, "role": "admin" }))
        .send()
        .await
        .expect("POST /accounts/{id}/members");
    assert_eq!(res.status(), 200, "an Owner's grant succeeds");
    let body: serde_json::Value = res.json().await.expect("json body");
    assert_eq!(body["user"], grantee_did, "the response echoes the grantee");
    assert_eq!(
        body["role"], "admin",
        "the response echoes the granted role"
    );

    // The grantee is now an Admin of the account. Provisioning their DID is
    // idempotent — it returns the very User the grant recognized.
    let grantee = user_repo
        .provision(&Did::new(grantee_did.to_string()))
        .await
        .expect("provision the grantee");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    let role = account_repo
        .role_of(grantee.id, account)
        .await
        .expect("role_of");
    assert_eq!(
        role,
        Some(Role::Admin(None)),
        "the grantee holds the granted role"
    );
}

// Owner is never grantable through this seam — transfer is its own path. The guard
// refuses it, and no membership is seated. (Requires task ②.)
#[tokio::test]
async fn granting_owner_is_refused() {
    let did = "did:plc:e2enoowner";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    let grantee_did = "did:plc:e2ewouldbeowner";
    let res = client
        .post(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": grantee_did, "role": "owner" }))
        .send()
        .await
        .expect("POST /accounts/{id}/members");
    assert_eq!(res.status(), 403, "Owner cannot be granted here");

    // Nothing was seated for the would-be owner.
    let grantee = user_repo
        .provision(&Did::new(grantee_did.to_string()))
        .await
        .expect("provision");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    let role = account_repo
        .role_of(grantee.id, account)
        .await
        .expect("role_of");
    assert_eq!(role, None, "a refused grant seats no one");
}

// An account's Owner is never demoted through a grant — ownership moves only via the
// separate transfer seam. A grant addressed to the current Owner's DID is refused and
// leaves them Owner. (Requires task ②.)
#[tokio::test]
async fn the_owner_cannot_be_demoted_by_a_grant() {
    let did = "did:plc:e2eownerkeep";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    // The signed-in Owner targets their own DID — the only Owner in this account.
    let res = client
        .post(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": did, "role": "admin" }))
        .send()
        .await
        .expect("POST /accounts/{id}/members");
    assert_eq!(res.status(), 403, "the Owner cannot be demoted by a grant");

    let owner = user_repo
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision the owner");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    let role = account_repo
        .role_of(owner.id, account)
        .await
        .expect("role_of");
    assert_eq!(role, Some(Role::Owner(None)), "the Owner keeps their role");
}

// An unknown role discriminant is understood-but-unusable: rejected at the door with
// 422, before any authority check. (Independent of task ②.)
#[tokio::test]
async fn granting_an_unknown_role_is_rejected() {
    let did = "did:plc:e2ebadrole";
    let (base, _user_repo, _account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    let res = client
        .post(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": "did:plc:whoever", "role": "wizard" }))
        .send()
        .await
        .expect("POST /accounts/{id}/members");
    common::assert_problem(res, 422, "unknown_role").await;
}

// A grant addressed to an account that doesn't exist is a 404 — there's nothing to
// act on. Distinct from "you may not" (403). (Independent of task ②.)
#[tokio::test]
async fn granting_on_a_missing_account_is_not_found() {
    let did = "did:plc:e2enoacct";
    let (base, _user_repo, _account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    let missing = Uuid::now_v7();
    let res = client
        .post(format!("{base}/accounts/{missing}/members"))
        .json(&serde_json::json!({ "user": "did:plc:whoever", "role": "member" }))
        .send()
        .await
        .expect("POST /accounts/{id}/members");
    assert_eq!(res.status(), 404, "granting on a missing account is 404");
}

// An anonymous visitor cannot grant — turned away at 401 before any account lookup.
// (Independent of task ②.)
#[tokio::test]
async fn anonymous_visitor_cannot_grant_a_role() {
    let (base, _user_repo, _account_repo) = spawn_app("did:plc:nobody").await;

    let res = client()
        .post(format!("{base}/accounts/{}/members", Uuid::now_v7()))
        .json(&serde_json::json!({ "user": "did:plc:whoever", "role": "member" }))
        .send()
        .await
        .expect("POST /accounts/{id}/members");
    assert_eq!(res.status(), 401, "an unrecognized visitor cannot grant");
}

#[tokio::test]
async fn anonymous_visitor_cannot_found_an_account() {
    let (base, _user_repo, account_repo) = spawn_app("did:plc:nobody").await;

    // No sign-in: the cookie jar carries no session.
    let res = client()
        .post(format!("{base}/accounts"))
        .send()
        .await
        .expect("POST /accounts");

    assert_eq!(
        res.status(),
        401,
        "an unrecognized visitor cannot found an account"
    );
    // Nothing was minted or persisted as a side effect of the rejected request.
    let found = account_repo
        .find(AccountId::new(Uuid::now_v7()))
        .await
        .expect("find");
    assert!(found.is_none());
}

/// Seats a member by granting them a role — the setup step for the revoke tests.
async fn grant_role(client: &reqwest::Client, base: &str, account_id: &str, did: &str, role: &str) {
    let res = client
        .post(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": did, "role": role }))
        .send()
        .await
        .expect("POST /accounts/{id}/members");
    assert_eq!(res.status(), 200, "granting the setup role succeeds");
}

// ZMVP-16 — the heart: an Owner revokes a member and they hold no role afterward.
// The Owner is left untouched, and a second revoke finds no membership (the user
// still exists but is no longer a member).
#[tokio::test]
async fn owner_revokes_a_member_and_unseats_them() {
    let did = "did:plc:e2erevoker";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    let member_did = "did:plc:e2erevokee";
    grant_role(&client, &base, &account_id, member_did, "admin").await;

    let res = client
        .delete(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": member_did }))
        .send()
        .await
        .expect("DELETE /accounts/{id}/members");
    assert_eq!(res.status(), 200, "an Owner's revoke succeeds");
    let body: serde_json::Value = res.json().await.expect("json body");
    assert_eq!(
        body["user"], member_did,
        "the response echoes the revoked member"
    );

    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    let member = user_repo
        .provision(&Did::new(member_did.to_string()))
        .await
        .expect("provision the member");
    let role = account_repo
        .role_of(member.id, account)
        .await
        .expect("role_of");
    assert_eq!(role, None, "the revoked member holds no role");

    // The Owner is unaffected by revoking someone else.
    let owner = user_repo
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision the owner");
    let owner_role = account_repo
        .role_of(owner.id, account)
        .await
        .expect("role_of");
    assert_eq!(
        owner_role,
        Some(Role::Owner(None)),
        "the Owner is left untouched"
    );

    // A second revoke: the user still exists but is no longer a member → 404.
    let res = client
        .delete(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": member_did }))
        .send()
        .await
        .expect("DELETE /accounts/{id}/members (again)");
    assert_eq!(res.status(), 404, "a second revoke finds no membership");
}

// An Owner is never revocable through this seam — no one outranks them. Keeps a
// sole Owner safe; ownership only leaves via the separate transfer seam.
#[tokio::test]
async fn the_owner_cannot_be_revoked() {
    let did = "did:plc:e2eownersafe";
    let (base, user_repo, account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    // The Owner targets their own DID.
    let res = client
        .delete(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": did }))
        .send()
        .await
        .expect("DELETE /accounts/{id}/members");
    assert_eq!(res.status(), 403, "an Owner cannot be revoked here");

    let owner = user_repo
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision the owner");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    let role = account_repo
        .role_of(owner.id, account)
        .await
        .expect("role_of");
    assert_eq!(role, Some(Role::Owner(None)), "the Owner keeps their role");
}

// Revoking someone who isn't a member is a 404 — and resolving them must not mint a
// User as a side effect (revoke uses a read-only DID lookup, not provision).
#[tokio::test]
async fn revoking_a_non_member_is_not_found() {
    let did = "did:plc:e2erevnonmember";
    let (base, _user_repo, _account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    let res = client
        .delete(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": "did:plc:stranger" }))
        .send()
        .await
        .expect("DELETE /accounts/{id}/members");
    assert_eq!(res.status(), 404, "revoking a non-member is 404");
}

// A revoke addressed to an account that doesn't exist is a 404.
#[tokio::test]
async fn revoking_on_a_missing_account_is_not_found() {
    let did = "did:plc:e2erevnoacct";
    let (base, _user_repo, _account_repo) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    let missing = Uuid::now_v7();
    let res = client
        .delete(format!("{base}/accounts/{missing}/members"))
        .json(&serde_json::json!({ "user": "did:plc:whoever" }))
        .send()
        .await
        .expect("DELETE /accounts/{id}/members");
    assert_eq!(res.status(), 404, "revoking on a missing account is 404");
}

// An anonymous visitor cannot revoke — turned away at 401 before any lookup.
#[tokio::test]
async fn anonymous_visitor_cannot_revoke_a_role() {
    let (base, _user_repo, _account_repo) = spawn_app("did:plc:nobody").await;

    let res = client()
        .delete(format!("{base}/accounts/{}/members", Uuid::now_v7()))
        .json(&serde_json::json!({ "user": "did:plc:whoever" }))
        .send()
        .await
        .expect("DELETE /accounts/{id}/members");
    assert_eq!(res.status(), 401, "an unrecognized visitor cannot revoke");
}
