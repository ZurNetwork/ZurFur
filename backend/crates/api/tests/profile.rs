//! End-to-end: a signed-in visitor sees their handle, display name, and
//! avatar; repeat views are served from the cache without waking the PDS; and an
//! unreachable PDS degrades gracefully. Every dependency is faked in-process
//! (PDS, user store, profile source/cache, session store) so the whole `/me`
//! read-through is exercised without a network or a database.
use std::sync::Arc;

use adapter_mem::MemProfileSource;
use api::AppState;
use domain::elements::{did::Did, profile::Profile};
use test_support::http::{client, serve, sign_in};

#[tokio::test]
async fn me_shows_profile_then_serves_it_from_cache() {
    let did = "did:plc:e2ealice";

    // Typed handle to the source so we can count PDS reads. The fixture builds
    // its own internal MemProfileSource from the given Profile, so this test
    // overrides `profile_source` after `build()` to keep that handle around
    // for `fetch_count`/`set_unreachable` — no custom trait, just a swap of a
    // pub field on the built Runtime (justification: this test needs the
    // concrete adapter, not the `dyn ProfileSource` the fixture returns).
    let profile = Profile::new(Did::from(did.to_string()), "alice.bsky.social")
        .with_display_name("Alice")
        .with_avatar_url("https://pds.example/avatar/alice.jpg");
    let source = Arc::new(MemProfileSource::new(profile));
    let served = serve(test_support::runtime::mem(&Did::from(did.to_string())), {
        let source = source.clone();
        move |rt| {
            api::app(AppState {
                profile_source: source,
                ..rt
            })
        }
    })
    .await;
    let base = served.base_url;
    let client = client();
    sign_in(&client, &base, "alice.bsky.social").await;

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

    // The PDS is down and nothing is cached: the page must still load. Same
    // post-build override as the test above — the fixture has no way to hand
    // back a pre-configured (unreachable) MemProfileSource.
    let source = MemProfileSource::new(Profile::new(Did::from(did.to_string()), "bob.bsky.social"));
    source.set_unreachable();
    let source = Arc::new(source);
    let served = serve(
        test_support::runtime::mem(&Did::from(did.to_string())),
        move |rt| {
            api::app(AppState {
                profile_source: source,
                ..rt
            })
        },
    )
    .await;
    let base = served.base_url;
    let client = client();
    sign_in(&client, &base, "alice.bsky.social").await;

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
