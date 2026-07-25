//! `GET /accounts` (ZMVP-157) — every live account the signed-in visitor holds
//! a role in, each row carrying the caller's own role:
//!
//! - role-based listing includes a **non-Owner** membership, not just accounts
//!   the caller founded;
//! - soft-deleted accounts are excluded;
//! - ordering is deterministic (by id — UUIDv7 sorts as creation order);
//! - an anonymous caller gets a `401` problem+json, same as `GET /me`.
//!
//! Same in-process fakes as the other api e2e suites — no network, no database.

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
use tower_sessions::{MemoryStore, SessionManagerLayer};

mod common;
use common::assert_problem;

/// Boots the app with everything faked in-process; returns the base URL and the
/// [`MemBackend`] so a test can seed accounts/memberships and introspect them.
/// `did` is the identity `sign_in` authenticates as.
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
            handle: "lister.bsky.social".to_string(),
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
        .body("handle=lister.bsky.social")
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

/// Found an account for `owner_did` directly on the backend (test seed of
/// [`domain::ports::AccountWrites::create`]), skipping the DID-minting HTTP
/// round trip. Returns the founded [`Account`].
async fn seed_account(backend: &MemBackend, owner_did: &str, handle: &str) -> Account {
    let owner = backend
        .provision(&Did::new(owner_did.to_string()))
        .await
        .expect("provision owner");
    let (account, membership) = Account::open(
        owner.id,
        Did::new(format!("{owner_did}:acct")),
        Handle::try_new(handle).expect("valid handle"),
        "Seed Studio".parse::<AccountName>().expect("valid name"),
        Utc::now(),
    );
    backend
        .create(&account, &membership)
        .await
        .expect("seed account");
    account
}

// Role-based listing: GET /accounts returns every live account the caller
// holds a role in — an account they FOUNDED (Owner) and one they only hold a
// non-Owner role on via a grant (the accepted-invitation surface, ZMVP-20).
#[tokio::test]
async fn lists_every_live_account_the_caller_holds_a_role_in_with_that_role() {
    let did = "did:plc:lister";
    let (base, backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let me = backend
        .find_by_did(&Did::new(did.to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    // Found an account through the real HTTP surface — the caller is its Owner.
    let res = client
        .post(format!("{base}/accounts"))
        .json(&serde_json::json!({ "name": "Owned Studio", "handle": "owned.zurfur.app" }))
        .send()
        .await
        .expect("POST /accounts");
    assert_eq!(res.status(), 201);
    let founded: serde_json::Value = res.json().await.expect("json body");
    let owned_id = founded["id"].as_str().expect("id").to_string();

    // Seed a SECOND account owned by someone else, then grant the signed-in
    // caller a non-Owner role on it — the accepted-invitation shape (ZMVP-20).
    let granted = seed_account(&backend, "did:plc:someone-else", "granted.zurfur.app").await;
    backend
        .grant_role(&UserAccount {
            user_id: me.id,
            account_id: granted.id,
            role: Role::Member(None),
        })
        .await
        .expect("grant member role");

    // Seed a THIRD account the caller holds NO role in. This is the containment
    // case: without it, an implementation that listed every live account —
    // joined to whatever role the caller happened to hold — would return the
    // same two rows and pass. It is also the property the cross-persona
    // invariant (ZMVP-17) actually rests on, so it is not optional coverage.
    let strangers = seed_account(&backend, "did:plc:stranger", "stranger.zurfur.app").await;

    let res = client
        .get(format!("{base}/accounts"))
        .send()
        .await
        .expect("GET /accounts");
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.expect("json body");
    let rows = body.as_array().expect("array body");
    assert_eq!(rows.len(), 2, "both memberships are listed: {body}");

    let listed_ids: Vec<&str> = rows
        .iter()
        .map(|row| row["id"].as_str().expect("id"))
        .collect();
    assert!(
        !listed_ids.contains(&strangers.id.to_string().as_str()),
        "an account the caller holds no role in is never listed: {body}"
    );

    let owned_row = rows
        .iter()
        .find(|row| row["id"] == owned_id)
        .expect("the founded account is listed");
    assert_eq!(owned_row["role"], "owner", "the founder is Owner");
    assert_eq!(owned_row["handle"], "owned.zurfur.app");

    let granted_row = rows
        .iter()
        .find(|row| row["id"] == granted.id.to_string())
        .expect("the granted-only account is listed too — not owned-only");
    assert_eq!(
        granted_row["role"], "member",
        "the caller's OWN role rides along, not just presence"
    );
    assert_eq!(granted_row["handle"], "granted.zurfur.app");

    // Ordering is deterministic: ascending by account id (UUIDv7 sorts as
    // creation order) — independent of insertion order into the response.
    let mut expected_order: Vec<String> = vec![owned_id.clone(), granted.id.to_string()];
    expected_order.sort();
    let actual_order: Vec<String> = rows
        .iter()
        .map(|row| row["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(
        actual_order, expected_order,
        "rows are ordered ascending by id"
    );
}

// Soft-deleted accounts are excluded — mirrors `AccountStore::find`'s liveness
// semantics (DD 23003138): a tombstoned account the caller once held a role on
// must not appear, even though the membership row itself is left intact.
#[tokio::test]
async fn excludes_a_soft_deleted_account_the_caller_holds_a_role_in() {
    let did = "did:plc:lister2";
    let (base, backend) = spawn_app(did).await;
    let client = client();
    sign_in(&client, &base).await;
    let me = backend
        .find_by_did(&Did::new(did.to_string()))
        .await
        .expect("find me")
        .expect("signed in");

    // A live account the caller owns — must still show up.
    let live = seed_account(&backend, did, "live.zurfur.app").await;
    backend
        .grant_role(&UserAccount {
            user_id: me.id,
            account_id: live.id,
            role: Role::Owner(None),
        })
        .await
        .expect("seed live membership");

    // A second account the caller also holds a role on, then soft-deleted
    // through the real write path (never a bare-pool/backdoor mutation).
    let tombstoned = seed_account(&backend, "did:plc:tombstone-owner", "gone.zurfur.app").await;
    backend
        .grant_role(&UserAccount {
            user_id: me.id,
            account_id: tombstoned.id,
            role: Role::Member(None),
        })
        .await
        .expect("seed tombstoned membership");
    let mut uow = backend.database().begin().await.expect("begin");
    uow.accounts()
        .soft_delete(tombstoned.id)
        .await
        .expect("soft delete");
    uow.commit().await.expect("commit soft delete");

    let res = client
        .get(format!("{base}/accounts"))
        .send()
        .await
        .expect("GET /accounts");
    assert_eq!(res.status(), 200);
    let body: serde_json::Value = res.json().await.expect("json body");
    let rows = body.as_array().expect("array body");
    assert_eq!(
        rows.len(),
        1,
        "the soft-deleted account is excluded: {body}"
    );
    assert_eq!(rows[0]["id"], live.id.to_string());
}

// An anonymous caller gets a 401 problem+json, exactly like `GET /me` — never a
// redirect, since the frontend calls this endpoint.
#[tokio::test]
async fn anonymous_caller_is_turned_away_with_401() {
    let (base, _backend) = spawn_app("did:plc:lister3").await;

    let res = client()
        .get(format!("{base}/accounts"))
        .send()
        .await
        .expect("GET /accounts");
    assert_problem(res, 401, "not_authenticated").await;
}
