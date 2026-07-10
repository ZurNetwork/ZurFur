//! End-to-end account founding (ZMVP-14): a signed-in visitor POSTs `/accounts`,
//! the server mints the account's sovereign `did:plc`, founds the account, and makes
//! the creating User its Owner. An anonymous visitor is turned away. Same in-process
//! fakes as the sign-in e2e — no network, no database.
use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemDidMinter, MemProfileSource};
use api::{AppState, Config, Environment};
use chrono::Utc;
use domain::elements::{
    account::{Account, AccountId, AccountName},
    did::Did,
    handle::Handle,
    profile::Profile,
    role::Role,
    user_account::UserAccount,
};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};
use uuid::Uuid;

mod common;

/// Boots the app with everything faked in-process and returns the base URL plus
/// typed handles to the repos, so a test can introspect them after the flow. The
/// unsizing to the `Arc<dyn …>` fields happens at assignment.
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
            deadline_sweep_interval_secs: 60,
            max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
        },
        // No route here touches the database, so a lazy (never-connected) pool keeps
        // the test free of a container.
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
        files: backend.file_store(),
        did_minter: Arc::new(MemDidMinter::new()),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{addr}"), backend)
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
    let (base, backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    // Found the account — founding requires a name and a handle.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Acme Studio", "handle": "Acme.Zurfur.App" }))
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
    assert_eq!(
        body["handle"], "acme.zurfur.app",
        "the response echoes the normalized (lowercased) handle"
    );

    // The creating User is the Owner of the founded account (the heart of ZMVP-14).
    let user = backend
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision is idempotent — returns the signed-in User");
    let account = AccountId::new(Uuid::parse_str(account_id).expect("id is a uuid"));
    let role = backend.role_of(user.id, account).await.expect("role_of");
    assert_eq!(
        role,
        Some(Role::Owner(None)),
        "the creating User becomes the account's Owner"
    );

    // And the account itself is persisted, retrievable by id — under its minted DID
    // and its normalized handle.
    let found = backend
        .find(account)
        .await
        .expect("find")
        .expect("account is stored");
    assert_eq!(
        found.did,
        Did::new(account_did.to_string()),
        "the founded account is stored under its minted did"
    );
    assert_eq!(
        found.handle.as_str(),
        "acme.zurfur.app",
        "the founded account stores its normalized handle"
    );
}

#[tokio::test]
async fn founding_requires_a_name() {
    let did = "did:plc:e2enoname";
    let (base, _backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    // A blank name is understood but unusable — rejected with 422. The handle is
    // valid, so it is the name that fails (name is checked before the mint).
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "   ", "handle": "acme.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    common::assert_problem(res, 422, "invalid_request").await;

    // The rejected attempt minted nothing: the next, valid founding gets the very
    // first DID from the deterministic mem minter (`did:plc:mem000000`). Had the
    // blank attempt reached the minter, this would be `...mem000001`.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Acme Studio", "handle": "acme.zurfur.app" }))
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
    // A handle is required at founding; derive a valid one from the first word of
    // the name (each test has its own backend, so a per-name handle never collides).
    let handle = format!(
        "{}.zurfur.app",
        name.split_whitespace()
            .next()
            .unwrap_or("acct")
            .to_lowercase()
    );
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": name, "handle": handle }))
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

// ZMVP-34 — the Owner deletes their account. With no facts persisted yet (the
// `account_has_facts` seam is `false` until commission storage lands), deletion is a
// hard-delete: the account is removed and its handle freed. Returns 204.
#[tokio::test]
async fn owner_deletes_their_empty_account() {
    let did = "did:plc:e2edeleter";
    let (base, backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let account_id = found_account(&client, &base, "Acme Studio").await;

    let res = client
        .delete(format!("{base}/accounts/{account_id}"))
        .send()
        .await
        .expect("DELETE /accounts/{id}");
    assert_eq!(
        res.status(),
        204,
        "the Owner deleting their account returns 204 No Content"
    );

    // Empty → hard-deleted → gone.
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    assert!(
        backend.find(account).await.expect("find").is_none(),
        "the deleted account is gone"
    );
}

// A hard-deleted (empty) account frees its handle: a brand-new account may reclaim it.
// Only empty accounts hard-delete, so the freed label carries no reputation (DD 23003138).
#[tokio::test]
async fn deleting_an_empty_account_frees_its_handle() {
    let did = "did:plc:e2erefound";
    let (base, _backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    // Founds `acme.zurfur.app` (handle derived from the first word of the name).
    let account_id = found_account(&client, &base, "Acme Studio").await;

    let res = client
        .delete(format!("{base}/accounts/{account_id}"))
        .send()
        .await
        .expect("DELETE /accounts/{id}");
    assert_eq!(res.status(), 204);

    // The freed handle can be founded anew (a soft-delete would have kept it reserved →
    // 409; this is the hard-delete contrast).
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Acme Reborn", "handle": "acme.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts over the freed handle");
    assert_eq!(
        res.status(),
        201,
        "the freed handle may be reclaimed by a new account"
    );
}

// Deleting an account that isn't there (or was already deleted) is a 404 — there is
// nothing live to act on, kept distinct from a 403 "you may not".
#[tokio::test]
async fn deleting_an_unknown_account_is_404() {
    let did = "did:plc:e2edelmissing";
    let (base, _backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    let missing = Uuid::now_v7();
    let res = client
        .delete(format!("{base}/accounts/{missing}"))
        .send()
        .await
        .expect("DELETE /accounts/{id}");
    assert_eq!(res.status(), 404, "deleting an unknown account is 404");
}

// Deletion is a write, so an anonymous caller is turned away with 401 (a problem+json
// status, never a redirect) — before any account is loaded.
#[tokio::test]
async fn deleting_requires_a_signed_in_user() {
    let did = "did:plc:e2edelanon";
    let (base, _backend) = spawn_app(did).await;
    let client = client(); // deliberately not signed in

    let some_id = Uuid::now_v7();
    let res = client
        .delete(format!("{base}/accounts/{some_id}"))
        .send()
        .await
        .expect("DELETE /accounts/{id}");
    assert_eq!(
        res.status(),
        401,
        "an anonymous caller is turned away with 401"
    );
}

// Owner-only: a signed-in member who is NOT the Owner (here an Admin) is forbidden from
// deleting the account — 403, distinct from 401 (they are signed in) and 404 (the
// account is live). The e2e harness signs in only one DID, so the account is owned by
// another user and the signed-in caller is seated as a non-Owner member via the backend.
#[tokio::test]
async fn a_non_owner_member_cannot_delete() {
    let (base, backend) = spawn_app("did:plc:deleter-nonowner").await;
    let client = client();
    sign_in(&client, &base).await;

    let me = backend
        .find_by_did(&Did::new("did:plc:deleter-nonowner".to_string()))
        .await
        .expect("find me")
        .expect("sign-in provisioned me");
    let owner = backend
        .provision(&Did::new("did:plc:realowner".to_string()))
        .await
        .expect("provision owner");
    let (account, owner_membership) = Account::open(
        owner.id,
        Did::new("did:plc:ownedacct".to_string()),
        Handle::try_new("owned.zurfur.app").unwrap(),
        AccountName::try_new("Not Yours").unwrap(),
        Utc::now(),
    );
    backend
        .create(&account, &owner_membership)
        .await
        .expect("found the account under another owner");
    backend
        .grant_role(&UserAccount {
            user_id: me.id,
            account_id: account.id,
            role: Role::Admin(None),
        })
        .await
        .expect("seat me as a non-Owner Admin");

    let res = client
        .delete(format!("{base}/accounts/{}", *account.id))
        .send()
        .await
        .expect("DELETE /accounts/{id}");
    common::assert_problem(res, 403, "forbidden").await;

    // The account is untouched by the forbidden attempt.
    assert!(
        backend.find(account.id).await.expect("find").is_some(),
        "the account still exists after the forbidden delete"
    );
}

// ZMVP-15 — the heart: an Owner grants a role and the grantee is seated as a member
// of the account at that role. The grantee is named by DID and need not have signed
// in; the grant recognizes them. (Requires task ②, `Role::can_grant`, to be live.)
#[tokio::test]
async fn owner_grants_a_role_and_seats_the_member() {
    let did = "did:plc:e2egranter";
    let (base, backend) = spawn_app(did).await;
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
    let grantee = backend
        .provision(&Did::new(grantee_did.to_string()))
        .await
        .expect("provision the grantee");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    let role = backend.role_of(grantee.id, account).await.expect("role_of");
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
    let (base, backend) = spawn_app(did).await;
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
    let grantee = backend
        .provision(&Did::new(grantee_did.to_string()))
        .await
        .expect("provision");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    let role = backend.role_of(grantee.id, account).await.expect("role_of");
    assert_eq!(role, None, "a refused grant seats no one");
}

// An account's Owner is never demoted through a grant — ownership moves only via the
// separate transfer seam. A grant addressed to the current Owner's DID is refused and
// leaves them Owner. (Requires task ②.)
#[tokio::test]
async fn the_owner_cannot_be_demoted_by_a_grant() {
    let did = "did:plc:e2eownerkeep";
    let (base, backend) = spawn_app(did).await;
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

    let owner = backend
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision the owner");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    let role = backend.role_of(owner.id, account).await.expect("role_of");
    assert_eq!(role, Some(Role::Owner(None)), "the Owner keeps their role");
}

// An unknown role discriminant is understood-but-unusable: rejected at the door with
// 422, before any authority check. (Independent of task ②.)
#[tokio::test]
async fn granting_an_unknown_role_is_rejected() {
    let did = "did:plc:e2ebadrole";
    let (base, _backend) = spawn_app(did).await;
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
    let (base, _backend) = spawn_app(did).await;
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
    let (base, _backend) = spawn_app("did:plc:nobody").await;

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
    let (base, backend) = spawn_app("did:plc:nobody").await;

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
    let found = backend
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
    let (base, backend) = spawn_app(did).await;
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
    let member = backend
        .provision(&Did::new(member_did.to_string()))
        .await
        .expect("provision the member");
    let role = backend.role_of(member.id, account).await.expect("role_of");
    assert_eq!(role, None, "the revoked member holds no role");

    // The Owner is unaffected by revoking someone else.
    let owner = backend
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision the owner");
    let owner_role = backend.role_of(owner.id, account).await.expect("role_of");
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
    let (base, backend) = spawn_app(did).await;
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

    let owner = backend
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision the owner");
    let account = AccountId::new(Uuid::parse_str(&account_id).expect("id is a uuid"));
    let role = backend.role_of(owner.id, account).await.expect("role_of");
    assert_eq!(role, Some(Role::Owner(None)), "the Owner keeps their role");
}

// Revoking someone who isn't a member is a 404 — and resolving them must not mint a
// User as a side effect (revoke uses a read-only DID lookup, not provision).
#[tokio::test]
async fn revoking_a_non_member_is_not_found() {
    let did = "did:plc:e2erevnonmember";
    let (base, _backend) = spawn_app(did).await;
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
    let (base, _backend) = spawn_app(did).await;
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
    let (base, _backend) = spawn_app("did:plc:nobody").await;

    let res = client()
        .delete(format!("{base}/accounts/{}/members", Uuid::now_v7()))
        .json(&serde_json::json!({ "user": "did:plc:whoever" }))
        .send()
        .await
        .expect("DELETE /accounts/{id}/members");
    assert_eq!(res.status(), 401, "an unrecognized visitor cannot revoke");
}

// ZMVP-44 — handle validation at the founding claim site. A punycode handle is
// rejected by the shared `Handle` gate and mapped to a 422 problem+json
// (`invalid_request`), and nothing is minted.
#[tokio::test]
async fn founding_rejects_a_punycode_handle() {
    let did = "did:plc:e2epuny";
    let (base, _backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Acme Studio", "handle": "xn--80ak6aa92e.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    common::assert_problem(res, 422, "invalid_request").await;
}

// A reserved label in the Zurfur namespace is rejected the same way (the gate is
// the `Handle` newtype; ZMVP-45).
#[tokio::test]
async fn founding_rejects_a_reserved_handle() {
    let did = "did:plc:e2ereserved";
    let (base, _backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Admin", "handle": "admin.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    common::assert_problem(res, 422, "invalid_request").await;
}

// A handle already claimed by a live account is a 409 (`handle_taken`), and the
// rejected founding mints nothing — proven the same way as `founding_requires_a_name`:
// a subsequent valid founding still gets the *second* mem DID, so the 409 consumed
// none.
#[tokio::test]
async fn founding_rejects_a_duplicate_handle() {
    let did = "did:plc:e2edup";
    let (base, _backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    // First claim of `taken.zurfur.app` succeeds — it takes the first mem DID.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "First", "handle": "taken.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201, "the first claim of the handle succeeds");
    let first_did = res.json::<serde_json::Value>().await.expect("json")["did"]
        .as_str()
        .expect("did")
        .to_string();
    assert_eq!(
        first_did, "did:plc:mem000000",
        "the first founding mints the first DID"
    );

    // A second account claiming the same handle (any case — it normalizes to the
    // same value) is refused with 409 handle_taken.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Second", "handle": "Taken.Zurfur.App" }))
        .send()
        .await
        .expect("POST /accounts");
    common::assert_problem(res, 409, "handle_taken").await;

    // The rejected founding minted nothing: the next, valid founding gets the SECOND
    // mem DID (`...mem000001`). Had the 409 reached the minter, this would be `...002`.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Third", "handle": "fresh.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201);
    assert_eq!(
        res.json::<serde_json::Value>().await.expect("json")["did"],
        "did:plc:mem000001",
        "a rejected 409 founding must not consume a minted identity"
    );
}

// A handle reserved by a SOFT-DELETED (tombstoned) account is invisible to the
// resolver (404) but still reserves the handle: founding over it is a 409, NOT a
// 500 (the founding pre-check filters soft-deleted rows, so this exercises the
// store-level `HandleTaken` backstop — the global unique index, DD 23003138).
#[tokio::test]
async fn founding_over_a_soft_deleted_handle_is_409_not_500() {
    let did = "did:plc:e2etombstone";
    let (base, backend) = spawn_app(did).await;

    // Seed a tombstoned account holding `gone.zurfur.app` (no soft-delete write path
    // exists yet, so insert it directly — the mem mirror of an UPDATE deleted_at).
    let reserved = domain::elements::handle::Handle::try_new("gone.zurfur.app").unwrap();
    backend.seed_soft_deleted_account(&Did::new("did:plc:tombstoned".to_string()), &reserved);

    // The resolver does not serve a tombstoned handle.
    let res = client()
        .get(format!("{base}/.well-known/atproto-did"))
        .header(reqwest::header::HOST, "gone.zurfur.app")
        .send()
        .await
        .expect("GET /.well-known/atproto-did");
    assert_eq!(res.status(), 404, "a tombstoned handle does not resolve");

    // ...but founding over it is a clean 409 (the store backstop), not a 500.
    let client = client();
    sign_in(&client, &base).await;
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Reclaimer", "handle": "gone.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    common::assert_problem(res, 409, "handle_taken").await;
}

// ZMVP-44 — atproto handle resolution. A GET to `/.well-known/atproto-did` with the
// handle in the `Host` header returns the account's bare `did:plc` as text/plain 200.
#[tokio::test]
async fn wellknown_resolves_a_zurfur_handle_to_its_did() {
    let did = "did:plc:e2eresolve";
    let (base, _backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    // Found an account under `alice.zurfur.app` and capture its minted DID.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Alice", "handle": "alice.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201);
    let account_did = res.json::<serde_json::Value>().await.expect("json")["did"]
        .as_str()
        .expect("did")
        .to_string();

    // A resolver fetches `/.well-known/atproto-did` with the handle as the Host.
    let res = client
        .get(format!("{base}/.well-known/atproto-did"))
        .header(reqwest::header::HOST, "alice.zurfur.app")
        .send()
        .await
        .expect("GET /.well-known/atproto-did");
    assert_eq!(res.status(), 200, "a known handle resolves");
    assert!(
        res.headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.starts_with("text/plain")),
        "the DID is served as text/plain",
    );
    let body = res.text().await.expect("text body");
    assert_eq!(body, account_did, "the bare DID is returned, unwrapped");
}

// An unknown handle under the Zurfur namespace resolves to 404 (no such account).
#[tokio::test]
async fn wellknown_unknown_handle_is_404() {
    let (base, _backend) = spawn_app("did:plc:nobody").await;

    let res = client()
        .get(format!("{base}/.well-known/atproto-did"))
        .header(reqwest::header::HOST, "nobody.zurfur.app")
        .send()
        .await
        .expect("GET /.well-known/atproto-did");
    assert_eq!(res.status(), 404, "an unknown handle does not resolve");
}

// A `Host` outside the configured handle domain is never served — even when an
// account holds exactly that (BYO) handle, because a BYO handle resolves at the
// owner's own domain, not from Zurfur's well-known.
#[tokio::test]
async fn wellknown_does_not_serve_a_foreign_host() {
    let did = "did:plc:e2ebyo";
    let (base, _backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;

    // Found an account under a brought (BYO) domain.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Alice", "handle": "alice.example.com" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201);

    // The well-known route refuses a Host outside `zurfur.app` up front (404),
    // never looking the account up.
    let res = client
        .get(format!("{base}/.well-known/atproto-did"))
        .header(reqwest::header::HOST, "alice.example.com")
        .send()
        .await
        .expect("GET /.well-known/atproto-did");
    assert_eq!(res.status(), 404, "a foreign Host is not ours to resolve");
}
