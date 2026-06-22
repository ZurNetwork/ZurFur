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
    assert_eq!(res.status(), 422, "a blank name is rejected");

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
