//! ZMVP-10 end-to-end: a signed-in visitor sees their handle, display name, and
//! avatar; repeat views are served from the cache without waking the PDS; and an
//! unreachable PDS degrades gracefully. Every dependency is faked in-process
//! (PDS, user store, profile source/cache, session store) so the whole `/me`
//! read-through is exercised without a network or a database.
use std::sync::Arc;

use adapter_mem::MemProfileSource;
use api::AppState;
use domain::elements::{did::Did, profile::Profile};
use reqwest::redirect::Policy;
use tower_sessions::{MemoryStore, SessionManagerLayer};

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

    // Typed handle to the source so we can count PDS reads. The fixture builds
    // its own internal MemProfileSource from the given Profile, so this test
    // overrides `profile_source` after `build()` to keep that handle around
    // for `fetch_count`/`set_unreachable` — no custom trait, just a swap of a
    // pub field on the built Runtime (justification: this test needs the
    // concrete adapter, not the `dyn ProfileSource` the fixture returns).
    let profile = Profile::new(Did::new(did.to_string()), "alice.bsky.social")
        .with_display_name("Alice")
        .with_avatar_url("https://pds.example/avatar/alice.jpg");
    let source = Arc::new(MemProfileSource::new(profile));
    let test_support::runtime::MemRuntime { mut runtime, .. } =
        test_support::runtime::mem(&Did::new(did.to_string()))
            .public_url(format!("http://{addr}"))
            .build();
    runtime.profile_source = source.clone();
    let state: AppState = runtime;
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

    // The PDS is down and nothing is cached: the page must still load. Same
    // post-build override as the test above — the fixture has no way to hand
    // back a pre-configured (unreachable) MemProfileSource.
    let source = MemProfileSource::new(Profile::new(Did::new(did.to_string()), "bob.bsky.social"));
    source.set_unreachable();
    let test_support::runtime::MemRuntime { mut runtime, .. } =
        test_support::runtime::mem(&Did::new(did.to_string()))
            .public_url(format!("http://{addr}"))
            .build();
    runtime.profile_source = Arc::new(source);
    let state: AppState = runtime;
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
