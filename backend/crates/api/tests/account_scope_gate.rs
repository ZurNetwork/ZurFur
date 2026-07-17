//! ZMVP-47 — capability-scoped write gating, end to end. The account-scope
//! authorization floor is now one shared `AccountRole` extractor every
//! account-scoped write flows through; this pins the three tiers of authority it
//! enforces, plus the two things it must **not** do:
//!
//! - **anonymous → 401** on any account-scoped write (read-only for the signed-out);
//! - **authed User with no role on the target Account → 403** `forbidden` (problem+json),
//!   and the attempt mutates nothing;
//! - **authed User holding the requisite role → success** (the gate isn't a blanket deny);
//! - **authed User with zero Accounts → a *user-scoped* write still succeeds** (founding
//!   an account is not account-scoped, so the gate is not applied — DD 26247170 §5);
//! - **anonymous read of an account's public data still succeeds** — the write gate must
//!   never close a public read path (discovery; DD 26247170 §5).
//!
//! Same in-process fakes as the other account e2e suites — no network, no database.

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
};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};
use uuid::Uuid;

mod common;

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] so a test can seed/introspect accounts and memberships directly.
/// `did` is the identity `sign_in` will authenticate as.
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
            did_key_root_key: "unused-in-tests".to_string(),
            plc_directory_endpoint: "https://plc.directory".to_string(),
            plc_directory_submit: false,
            deadline_sweep_interval_secs: 60,
            max_upload_bytes: Config::DEFAULT_MAX_UPLOAD_BYTES,
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
        files: backend.file_store(),
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

/// Drives the two-step sign-in so the client's cookie jar carries a live session
/// for the app's configured DID.
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

/// Founds an account through `POST /accounts` with the given session, returning its
/// id — the caller becomes its Owner.
async fn found_account(client: &reqwest::Client, base: &str, name: &str, handle: &str) -> String {
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": name, "handle": handle }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201, "founding an account returns 201");
    let body: serde_json::Value = res.json().await.expect("json body");
    body["id"].as_str().expect("account id").to_string()
}

/// Seed an Account owned by a freshly-provisioned user, directly through the backend
/// (no HTTP) — the signed-in test user is deliberately *not* a member of it.
async fn seed_foreign_account(backend: &MemBackend, owner_did: &str, handle: &str) -> Account {
    let owner = backend
        .provision(&Did::new(owner_did.to_string()))
        .await
        .expect("provision the foreign owner");
    let (account, membership) = Account::open(
        owner.id,
        Did::new(format!("{owner_did}:acct")),
        Handle::try_new(handle).expect("valid handle"),
        "Host Studio".parse::<AccountName>().expect("valid name"),
        Utc::now(),
    );
    backend
        .create(&account, &membership)
        .await
        .expect("seed the foreign account");
    account
}

// Tier 1 — an anonymous (signed-out) caller cannot make an account-scoped write:
// turned away at 401 `not_authenticated` (problem+json), read-only for the signed-out.
#[tokio::test]
async fn anonymous_cannot_make_an_account_scoped_write() {
    let (base, _backend) = spawn_app("did:plc:nobody").await;

    // No sign-in: the cookie jar carries no session. A grant is an account-scoped write.
    let res = client()
        .post(format!("{base}/accounts/{}/members", Uuid::now_v7()))
        .json(&serde_json::json!({ "user": "did:plc:whoever", "role": "member" }))
        .send()
        .await
        .expect("POST /accounts/{id}/members");
    common::assert_problem(res, 401, "not_authenticated").await;
}

// Tier 2 — an authenticated User with NO role on the target Account is rejected 403
// `forbidden` (problem+json) on an account-scoped write, and the attempt mutates
// nothing (they are still not a member afterward).
#[tokio::test]
async fn authed_user_without_a_role_is_forbidden_on_an_account_scoped_write() {
    let (base, backend) = spawn_app("did:plc:stranger").await;
    let client = client();
    sign_in(&client, &base).await; // signed in as did:plc:stranger

    // An account owned by someone else; the signed-in stranger holds no role on it.
    let account = seed_foreign_account(&backend, "did:plc:host", "host.zurfur.app").await;

    let res = client
        .post(format!("{base}/accounts/{}/members", *account.id))
        .json(&serde_json::json!({ "user": "did:plc:whoever", "role": "member" }))
        .send()
        .await
        .expect("POST /accounts/{id}/members");
    common::assert_problem(res, 403, "forbidden").await;

    // The forbidden write changed nothing. The gate rejects at the `AccountRole`
    // extractor — before the handler body runs at all — so nothing downstream (body
    // parse, the rank check, and crucially the grantee `provision`) ever executes.
    // Assert both halves: the stranger is still a non-member, AND the would-be grantee
    // was NOT provisioned as a side effect (grant_role recognizes grantees by DID, so a
    // leak here would mint a User the forbidden request should never have created).
    let me = backend
        .find_by_did(&Did::new("did:plc:stranger".to_string()))
        .await
        .expect("find me")
        .expect("sign-in provisioned me");
    assert_eq!(
        backend.role_of(me.id, account.id).await.expect("role_of"),
        None,
        "a forbidden write seats no membership for the actor",
    );
    assert!(
        backend
            .find_by_did(&Did::new("did:plc:whoever".to_string()))
            .await
            .expect("find grantee")
            .is_none(),
        "a forbidden grant provisions no User for the grantee (rejected before provisioning)",
    );
}

// Tier 3 — an authenticated User holding the requisite role succeeds on the
// account-scoped write: the gate is a floor, not a blanket deny.
#[tokio::test]
async fn authed_user_with_the_role_succeeds_on_an_account_scoped_write() {
    let (base, backend) = spawn_app("did:plc:owner").await;
    let client = client();
    sign_in(&client, &base).await;

    // The signed-in user founds the account and is its Owner — an Owner may grant.
    let account_id = found_account(&client, &base, "Acme Studio", "acme.zurfur.app").await;

    let res = client
        .post(format!("{base}/accounts/{account_id}/members"))
        .json(&serde_json::json!({ "user": "did:plc:grantee", "role": "member" }))
        .send()
        .await
        .expect("POST /accounts/{id}/members");
    assert_eq!(res.status(), 200, "the Owner's grant succeeds");

    // The grant took effect — the grantee now holds the seated role.
    let grantee = backend
        .find_by_did(&Did::new("did:plc:grantee".to_string()))
        .await
        .expect("find grantee")
        .expect("the grant provisioned the grantee");
    let account = account_id_from(&account_id);
    assert_eq!(
        backend.role_of(grantee.id, account).await.expect("role_of"),
        Some(Role::Member(None)),
        "the grantee is seated as a Member",
    );
}

// AC2 — a signed-in User with ZERO accounts can still make a *user-scoped* write:
// founding an account is not account-scoped, so the `AccountRole` gate is not applied.
#[tokio::test]
async fn authed_user_with_zero_accounts_can_make_a_user_scoped_write() {
    let (base, backend) = spawn_app("did:plc:newcomer").await;
    let client = client();
    sign_in(&client, &base).await;

    // The signed-in user holds no accounts yet.
    let me = backend
        .find_by_did(&Did::new("did:plc:newcomer".to_string()))
        .await
        .expect("find me")
        .expect("sign-in provisioned me");

    // Founding — a user-scoped write — succeeds with no prior account.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "First Studio", "handle": "first.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(
        res.status(),
        201,
        "a zero-account User can found an account (user-scoped write is not gated)",
    );

    // And it seated them as the Owner of the new account.
    let body: serde_json::Value = res.json().await.expect("json body");
    let account = account_id_from(body["id"].as_str().expect("account id"));
    assert_eq!(
        backend.role_of(me.id, account).await.expect("role_of"),
        Some(Role::Owner(None)),
        "founding makes the zero-account User the Owner",
    );
}

// Fork-1 guard — the write gate must NOT close a public read path. An account's
// public data (its handle → did:plc resolution) stays anonymously readable, so the
// retrofit can't accidentally gate discovery.
#[tokio::test]
async fn anonymous_read_of_account_public_data_still_succeeds() {
    let (base, _backend) = spawn_app("did:plc:owner").await;
    let owner_client = client();
    sign_in(&owner_client, &base).await;

    // Found an account so its handle → DID mapping exists in the store.
    found_account(&owner_client, &base, "Acme Studio", "acme.zurfur.app").await;

    // A brand-new client with NO session resolves the account's public DID by handle
    // (the `Host` header carries the handle, per DD/26607618). This must succeed.
    let res = client()
        .get(format!("{base}/.well-known/atproto-did"))
        .header(reqwest::header::HOST, "acme.zurfur.app")
        .send()
        .await
        .expect("GET /.well-known/atproto-did");
    assert_eq!(
        res.status(),
        200,
        "an account's public handle → DID read stays anonymous",
    );
    let did = res.text().await.expect("bare did body");
    assert!(
        did.starts_with("did:plc:"),
        "the resolver returns the account's did:plc, got {did:?}",
    );
}

/// Parse an account-id string (as returned by the API) back into an `AccountId` for
/// backend introspection.
fn account_id_from(id: &str) -> domain::elements::account::AccountId {
    domain::elements::account::AccountId::new(Uuid::parse_str(id).expect("id is a uuid"))
}
