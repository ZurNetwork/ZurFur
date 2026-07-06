//! ZMVP-46: an Owner changes an Account's handle after onboarding
//! (`PATCH /accounts/{id}/handle`), DD "Account Handle Change Flow" 27852802.
//!
//! Drives the whole handler against the in-process fakes: Owner-only authority (§2),
//! re-validation to the founding gate (Done-when), the light rate limit (§3), the
//! vacated-handle quarantine enforced at BOTH claim sites (§4), and the BYO-target
//! deferral (§6). Resolution following the change — `find_did_by_handle(new)` resolves
//! and `find_did_by_handle(old)` stops — is asserted through the shared store the
//! handler wrote. The `did:plc` `alsoKnownAs` re-point (§5/§7) is the ZMVP-50 op,
//! exercised in `adapter-atproto`'s own tests; here the mem minter stands in for it.
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
use serde_json::{Value, json};
use tower_sessions::{MemoryStore, SessionManagerLayer};
use uuid::Uuid;

mod common;

/// Boots the app with everything faked in-process; returns the base URL plus the repo
/// handles so a test can seed and introspect directly. The signed-in user (via
/// [`sign_in`]) resolves to `did`.
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

/// `PATCH /accounts/{id}/handle` with `{ "handle": new }`.
async fn change(client: &reqwest::Client, base: &str, id: &str, new: &str) -> reqwest::Response {
    client
        .patch(format!("{base}/accounts/{id}/handle"))
        .json(&json!({ "handle": new }))
        .send()
        .await
        .expect("PATCH handle")
}

fn handle(h: &str) -> Handle {
    Handle::try_new(h).expect("valid handle")
}

// AC (Done-when) — an Owner changes the handle and BOTH resolution halves follow: the
// new handle resolves to the account's DID, the old handle stops resolving.
#[tokio::test]
async fn owner_changes_handle_and_resolution_follows() {
    let (base, backend) = spawn_app("did:plc:changeowner").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = found_account(&client, &base, "Rename Studio", "before.zurfur.app").await;

    // The account's DID, captured before the change so we can assert resolution moves.
    let did = backend
        .find(AccountId::new(Uuid::parse_str(&id).unwrap()))
        .await
        .expect("find")
        .expect("account present")
        .did;

    let res = change(&client, &base, &id, "after.zurfur.app").await;
    assert_eq!(res.status(), 200, "the Owner's change succeeds");
    let body: Value = res.json().await.expect("json");
    assert_eq!(body["handle"].as_str(), Some("after.zurfur.app"));
    assert_eq!(body["id"].as_str(), Some(id.as_str()));
    assert_eq!(body["did"].as_str(), Some(did.as_str()));

    // handle→DID resolution follows: new resolves to the DID, old no longer resolves.
    let store = backend.account_store();
    assert_eq!(
        store
            .find_did_by_handle(&handle("after.zurfur.app"))
            .await
            .expect("resolve new"),
        Some(did.clone()),
        "the new handle resolves to the account's DID"
    );
    assert_eq!(
        store
            .find_did_by_handle(&handle("before.zurfur.app"))
            .await
            .expect("resolve old"),
        None,
        "the old handle no longer resolves"
    );
}

// §4 — the vacated *.zurfur.app handle is QUARANTINED to the account: after A vacates
// `taken.zurfur.app`, neither founding a NEW account on it nor a *different* account
// changing to it succeeds (409) — but the account that left it may RECLAIM it.
#[tokio::test]
async fn vacated_handle_is_quarantined_and_reclaimable() {
    let (base, _backend) = spawn_app("did:plc:quarowner").await;
    let client = client();
    sign_in(&client, &base).await;

    // Account 1 vacates `taken.zurfur.app` → it is quarantined to account 1.
    let a1 = found_account(&client, &base, "First", "taken.zurfur.app").await;
    assert_eq!(
        change(&client, &base, &a1, "moved.zurfur.app")
            .await
            .status(),
        200
    );

    // Founding a fresh account on the just-vacated handle is refused (§4, enforced at
    // the founding claim site too).
    let res = client
        .post(format!("{base}/accounts"))
        .json(&json!({ "name": "Squatter", "handle": "taken.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    common::assert_problem(res, 409, "handle_taken").await;

    // A DIFFERENT account (same owner may own several) also cannot change to it.
    let a2 = found_account(&client, &base, "Second", "second.zurfur.app").await;
    common::assert_problem(
        change(&client, &base, &a2, "taken.zurfur.app").await,
        409,
        "handle_taken",
    )
    .await;

    // ...but the account that vacated it may reclaim it within the window.
    assert_eq!(
        change(&client, &base, &a1, "taken.zurfur.app")
            .await
            .status(),
        200,
        "the account that left a handle may reclaim its own quarantined handle"
    );
}

// §2 — Owner-only: a mere member of the account cannot change its handle (403).
#[tokio::test]
async fn only_the_owner_may_change_the_handle() {
    let (base, backend) = spawn_app("did:plc:memberonly").await;
    let client = client();
    sign_in(&client, &base).await;

    // The signed-in user is only a Member of an account someone else owns.
    let me = backend
        .find_by_did(&Did::new("did:plc:memberonly".to_string()))
        .await
        .expect("find me")
        .expect("provisioned me");
    let host = backend
        .provision(&Did::new("did:plc:host".to_string()))
        .await
        .expect("provision host");
    let (account, owner_membership) = Account::open(
        host.id,
        Did::new("did:plc:hostacct".to_string()),
        handle("host.zurfur.app"),
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

    common::assert_problem(
        change(&client, &base, &account.id.to_string(), "hijack.zurfur.app").await,
        403,
        "forbidden",
    )
    .await;
}

// Done-when ("same validation guarantees as the initial claim") — the change re-runs
// the shared `Handle` gate: a reserved label / punycode / malformed target is 422.
#[tokio::test]
async fn rejects_an_invalid_handle() {
    let (base, _backend) = spawn_app("did:plc:invalidowner").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = found_account(&client, &base, "Studio", "valid.zurfur.app").await;

    // A reserved label in the Zurfur namespace (ZMVP-45) — same gate as founding.
    common::assert_problem(
        change(&client, &base, &id, "admin.zurfur.app").await,
        422,
        "invalid_request",
    )
    .await;
    // Punycode (ZMVP-48).
    common::assert_problem(
        change(&client, &base, &id, "xn--80ak6aa92e.zurfur.app").await,
        422,
        "invalid_request",
    )
    .await;
}

// §4 backstop — changing to a handle a LIVE account already holds is a 409, not a 500.
#[tokio::test]
async fn rejects_a_taken_handle() {
    let (base, backend) = spawn_app("did:plc:takenowner").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = found_account(&client, &base, "Mine", "mine.zurfur.app").await;

    // Seed another live account holding `theirs.zurfur.app`.
    let other = backend
        .provision(&Did::new("did:plc:otherowner".to_string()))
        .await
        .expect("provision other");
    let (account, membership) = Account::open(
        other.id,
        Did::new("did:plc:otheracct".to_string()),
        handle("theirs.zurfur.app"),
        AccountName::try_new("Theirs").unwrap(),
        Utc::now(),
    );
    backend
        .create(&account, &membership)
        .await
        .expect("seed other");

    common::assert_problem(
        change(&client, &base, &id, "theirs.zurfur.app").await,
        409,
        "handle_taken",
    )
    .await;
}

// §6 — BYO deferred: changing TO a brought (non-*.zurfur.app) handle is refused with
// the distinct `unsupported_handle` code until BYO re-binding ships.
#[tokio::test]
async fn rejects_a_byo_target() {
    let (base, _backend) = spawn_app("did:plc:byoowner").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = found_account(&client, &base, "Studio", "onus.zurfur.app").await;

    common::assert_problem(
        change(&client, &base, &id, "alice.example.com").await,
        422,
        "unsupported_handle",
    )
    .await;
}

// A no-op change (the account's own current handle) is rejected as unusable rather than
// consuming a rate-limit slot or signing a redundant op.
#[tokio::test]
async fn rejects_the_current_handle() {
    let (base, _backend) = spawn_app("did:plc:sameowner").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = found_account(&client, &base, "Studio", "same.zurfur.app").await;

    common::assert_problem(
        change(&client, &base, &id, "same.zurfur.app").await,
        422,
        "invalid_request",
    )
    .await;
}

// §3 — the light rate limit: after the ceiling of changes within the window, the next
// change is refused with 429 `rate_limited`.
#[tokio::test]
async fn rate_limits_rapid_changes() {
    let (base, _backend) = spawn_app("did:plc:rateowner").await;
    let client = client();
    sign_in(&client, &base).await;
    let id = found_account(&client, &base, "Busy", "rate0.zurfur.app").await;

    // Ten changes (the ceiling) all succeed, to distinct fresh handles.
    for i in 1..=10 {
        assert_eq!(
            change(&client, &base, &id, &format!("rate{i}.zurfur.app"))
                .await
                .status(),
            200,
            "change #{i} within the limit succeeds"
        );
    }
    // The eleventh trips the throttle.
    common::assert_problem(
        change(&client, &base, &id, "rate11.zurfur.app").await,
        429,
        "rate_limited",
    )
    .await;
}

// The auth floor: an anonymous caller is 401 (before any account lookup), and a missing
// account is 404.
#[tokio::test]
async fn anonymous_is_unauthorized_and_missing_account_is_not_found() {
    let (base, _backend) = spawn_app("did:plc:floorowner").await;
    let anon = client();

    // No session → 401, even for a nonexistent account.
    common::assert_problem(
        change(&anon, &base, &Uuid::now_v7().to_string(), "x.zurfur.app").await,
        401,
        "not_authenticated",
    )
    .await;

    // Signed in, but the account doesn't exist → 404.
    let client = client();
    sign_in(&client, &base).await;
    common::assert_problem(
        change(&client, &base, &Uuid::now_v7().to_string(), "x.zurfur.app").await,
        404,
        "account_not_found",
    )
    .await;
}
