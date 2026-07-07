//! ZMVP-10 end-to-end: a signed-in visitor sees their handle, display name, and
//! avatar; repeat views are served from the cache without waking the PDS; and an
//! unreachable PDS degrades gracefully. Every dependency is faked in-process
//! (PDS, user store, profile source/cache, session store) so the whole `/me`
//! read-through is exercised without a network or a database.
use std::sync::Arc;

use adapter_mem::{MemAuthenticator, MemBackend, MemProfileSource};
use api::{AppState, Config, Environment};
use domain::elements::{did::Did, profile::Profile};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};

fn config_for(addr: std::net::SocketAddr) -> Config {
    Config {
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
    }
}

/// Sign in through the faked OAuth handshake; leaves the client holding the session
/// cookie. Returns once `/me` is reachable as the signed-in visitor.
async fn sign_in(client: &reqwest::Client, base: &str) {
    let res = client
        .post(format!("{base}/signin"))
        .header("content-type", "application/x-www-form-urlencoded")
        .body("handle=alice.bsky.social")
        .send()
        .await
        .expect("POST /signin");
    assert_eq!(res.status(), 303);
    let res = client
        .get(format!("{base}/signin-callback?code=test"))
        .send()
        .await
        .expect("GET /signin-callback");
    assert_eq!(res.status(), 303);
}

#[tokio::test]
async fn me_shows_profile_then_serves_it_from_cache() {
    let did = "did:plc:e2ealice";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");

    // Typed handle to the source so we can count PDS reads.
    let source = Arc::new(MemProfileSource::new(Profile {
        did: Did::new(did.to_string()),
        handle: "alice.bsky.social".to_string(),
        display_name: Some("Alice".to_string()),
        avatar_url: Some("https://pds.example/avatar/alice.jpg".to_string()),
    }));
    let backend = MemBackend::new();
    let state = AppState {
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        did_minter: Arc::new(adapter_mem::MemDidMinter::new()),
        config: config_for(addr),
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new(did.to_string()))),
        users: backend.user_store(),
        profile_source: source.clone(),
        profile_cache: backend.profile_cache(),
        database: backend.database(),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client");
    let base = format!("http://{addr}");
    sign_in(&client, &base).await;

    // 1. First view: handle, display name, and avatar are all shown (criterion 1),
    //    and it took exactly one PDS read.
    let body = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me")
        .text()
        .await
        .expect("body");
    assert!(body.contains("alice.bsky.social"), "handle shown: {body}");
    assert!(body.contains("Alice"), "display name shown: {body}");
    assert!(
        body.contains("https://pds.example/avatar/alice.jpg"),
        "avatar shown: {body}"
    );
    assert_eq!(source.fetch_count(), 1, "first view reads the PDS once");

    // 2. Repeat view: served from the cache, no second PDS read (criterion 2).
    let body = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me")
        .text()
        .await
        .expect("body");
    assert!(body.contains("alice.bsky.social"));
    assert_eq!(
        source.fetch_count(),
        1,
        "a repeat view must not wake the PDS again"
    );

    // 3. PDS goes down after caching — the cached profile still renders (criterion 3).
    source.set_unreachable();
    let body = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me")
        .text()
        .await
        .expect("body");
    assert!(
        body.contains("alice.bsky.social"),
        "cached profile survives an unreachable PDS: {body}"
    );
}

#[tokio::test]
async fn me_degrades_to_did_when_pds_unreachable_and_uncached() {
    let did = "did:plc:e2ebob";
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");

    // The PDS is down and nothing is cached: the page must still load.
    let source = MemProfileSource::new(Profile {
        did: Did::new(did.to_string()),
        handle: "bob.bsky.social".to_string(),
        display_name: None,
        avatar_url: None,
    });
    source.set_unreachable();
    let backend = MemBackend::new();
    let state = AppState {
        accounts: backend.account_store(),
        commissions: backend.commission_store(),
        changelog: backend.changelog_store(),
        did_minter: Arc::new(adapter_mem::MemDidMinter::new()),
        config: config_for(addr),
        pool: adapter_pg::lazy_pool("postgres://unused/unused").expect("lazy pool"),
        auth: Arc::new(MemAuthenticator::new(Did::new(did.to_string()))),
        users: backend.user_store(),
        profile_source: Arc::new(source),
        profile_cache: backend.profile_cache(),
        database: backend.database(),
    };
    let app = api::app(state).layer(SessionManagerLayer::new(MemoryStore::default()));
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let client = reqwest::Client::builder()
        .cookie_store(true)
        .redirect(Policy::none())
        .build()
        .expect("client");
    let base = format!("http://{addr}");
    sign_in(&client, &base).await;

    let res = client
        .get(format!("{base}/me"))
        .send()
        .await
        .expect("GET /me");
    assert_eq!(res.status(), 200, "an unreachable PDS is not an error");
    let body = res.text().await.expect("body");
    assert!(
        body.contains(did),
        "degrades to showing the DID when no profile is available: {body}"
    );
}
