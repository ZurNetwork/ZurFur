//! `Users::me` over the in-memory fakes: the one implementation both drivers
//! call.

use std::future::Future as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use adapter_mem::MemProfileSource;
use application::user::me::{MeError, MeProfile, MeQuery};
use application::user::{LOOKUP_CAP, PROFILE_WAIT, ProfileLookups};
use async_trait::async_trait;
use composition::Runtime;
use domain::elements::{
    did::Did,
    profile::{DisplayHandle, Profile},
    user::UserId,
};
use domain::ports::{ProfileCache, ProfileSource, UnitOfWork};
use tokio::time::Instant;
use uuid::Uuid;

#[tokio::test]
async fn a_recognized_user_gets_their_profile() {
    let did = Did::from("did:plc:app-me".to_string());
    let profile = Profile::new(did.clone(), "me.bsky.social").with_display_name("Me");
    let fixture = test_support::runtime::mem(&did)
        .profile(profile.clone())
        .build();
    let runtime = fixture.runtime;
    let provisioned = did.clone();
    let user = runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.users().provision(&provisioned).await
        })
        .await
        .unwrap();

    let query = MeQuery { user_id: user.id };
    let answer = runtime
        .app()
        .users()
        .me(query, &runtime.profile_lookups)
        .await
        .unwrap();

    // Post DD 57081857 `me`'s reported id IS the caller's DID (no separate
    // `did` field on the output) — the round-trip the old assertion checked.
    assert_eq!(answer.id, UserId::from(did));
    let expected_profile = MeProfile {
        handle: DisplayHandle::from("me.bsky.social"),
        display_name: Some("Me".to_string()),
        avatar_url: None,
    };
    assert_eq!(answer.profile, Some(expected_profile));
}

#[tokio::test]
async fn an_unknown_id_is_unknown_user() {
    let did = Did::from("did:plc:app-nobody".to_string());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    // A UserId is a Did now (DD 57081857) — mint a fresh, distinct one rather
    // than wrapping a bare UUID.
    let id = UserId::from(Did::from(format!("did:plc:{}", Uuid::now_v7())));

    let query = MeQuery {
        user_id: id.clone(),
    };
    let error = runtime
        .app()
        .users()
        .me(query, &runtime.profile_lookups)
        .await
        .unwrap_err();

    assert!(matches!(error, MeError::UnknownUser(unknown) if unknown == id));
}

#[test]
fn a_profile_flattens_every_optional() {
    let did = Did::from("did:plc:app-flat".to_string());
    let profile = Profile::new(did, "flat.bsky.social")
        .with_display_name("Flat")
        .with_avatar_url("https://cdn/avatar.png");

    let flat = MeProfile::from(profile);

    let expected = MeProfile {
        handle: DisplayHandle::from("flat.bsky.social"),
        display_name: Some("Flat".to_string()),
        avatar_url: Some("https://cdn/avatar.png".to_string()),
    };
    assert_eq!(flat, expected);
}

// --- The bounded wait for the profile -------------------------------------------

/// The profile every source in this section answers with.
fn bounded_profile(did: &Did) -> Profile {
    Profile::new(did.clone(), "bounded.bsky.social").with_display_name("Bounded")
}

/// An in-memory runtime whose PDS is `source`, with the user behind `did`
/// already provisioned; returns the runtime and the user's id.
async fn runtime_over(did: &Did, source: Arc<dyn ProfileSource>) -> (Runtime, UserId) {
    let fixture = test_support::runtime::mem(did).build();
    let runtime = Runtime {
        profile_source: source,
        ..fixture.runtime
    };
    let provisioned = did.clone();
    let user = runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.users().provision(&provisioned).await
        })
        .await
        .expect("the user is provisioned");
    (runtime, user.id)
}

/// `me` for `user_id`; only the profile, since every caller here is known.
async fn me_profile(runtime: &Runtime, user_id: &UserId) -> Option<MeProfile> {
    let query = MeQuery {
        user_id: user_id.clone(),
    };
    let answer = runtime
        .app()
        .users()
        .me(query, &runtime.profile_lookups)
        .await
        .expect("a provisioned user is answered");
    answer.profile
}

#[test]
fn the_wait_is_two_seconds() {
    assert_eq!(PROFILE_WAIT, Duration::from_secs(2));
}

#[test]
fn the_cap_is_thirty_seconds() {
    assert_eq!(LOOKUP_CAP, Duration::from_secs(30));
}

#[tokio::test(start_paused = true)]
async fn a_source_that_never_answers_gets_the_bare_did_within_the_wait() {
    let did = Did::from("did:plc:app-silent".to_string());
    let source = Arc::new(MemProfileSource::new(bounded_profile(&did)));
    source.never_answer();
    let (runtime, user_id) = runtime_over(&did, source.clone()).await;

    let started = Instant::now();
    let profile = me_profile(&runtime, &user_id).await;

    assert_eq!(
        profile, None,
        "the wait ran out, so only the DID is reported"
    );
    assert_eq!(
        started.elapsed(),
        PROFILE_WAIT,
        "me waited exactly the wait"
    );
}

#[tokio::test(start_paused = true)]
async fn a_source_answering_inside_the_wait_is_shown_at_once() {
    let did = Did::from("did:plc:app-prompt".to_string());
    let source = Arc::new(MemProfileSource::new(bounded_profile(&did)));
    source.answer_after(PROFILE_WAIT / 2);
    let (runtime, user_id) = runtime_over(&did, source.clone()).await;

    let started = Instant::now();
    let profile = me_profile(&runtime, &user_id).await;

    let expected_profile = MeProfile::from(bounded_profile(&did));
    assert_eq!(profile, Some(expected_profile));
    assert_eq!(
        started.elapsed(),
        PROFILE_WAIT / 2,
        "me answered when the PDS did"
    );
}

#[tokio::test(start_paused = true)]
async fn a_slow_source_shows_its_profile_on_a_later_call() {
    let did = Did::from("did:plc:app-slow".to_string());
    let source = Arc::new(MemProfileSource::new(bounded_profile(&did)));
    source.answer_after(PROFILE_WAIT + Duration::from_secs(1));
    let (runtime, user_id) = runtime_over(&did, source.clone()).await;

    let first = me_profile(&runtime, &user_id).await;
    // Let the lookup that outlived the first call finish in the background.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let later = me_profile(&runtime, &user_id).await;

    let expected_profile = MeProfile::from(bounded_profile(&did));
    assert_eq!(first, None, "the first call ran out of wait");
    assert_eq!(
        later,
        Some(expected_profile),
        "the background fetch filled the cache"
    );
    assert_eq!(
        source.fetch_count(),
        1,
        "the later call read the cache, not the PDS"
    );
}

#[tokio::test(start_paused = true)]
async fn concurrent_misses_for_one_did_share_one_fetch() {
    let did = Did::from("did:plc:app-shared".to_string());
    let source = Arc::new(MemProfileSource::new(bounded_profile(&did)));
    source.answer_after(PROFILE_WAIT / 2);
    let (runtime, user_id) = runtime_over(&did, source.clone()).await;

    let (first, second) = tokio::join!(
        me_profile(&runtime, &user_id),
        me_profile(&runtime, &user_id)
    );

    let expected_profile = MeProfile::from(bounded_profile(&did));
    assert_eq!(first, Some(expected_profile.clone()));
    assert_eq!(second, Some(expected_profile));
    assert_eq!(source.fetch_count(), 1, "both misses joined one lookup");
}

#[tokio::test]
async fn a_failed_fetch_is_fetched_again_on_the_next_miss() {
    let did = Did::from("did:plc:app-flaky".to_string());
    let source = Arc::new(MemProfileSource::new(bounded_profile(&did)));
    source.set_unreachable();
    let (runtime, user_id) = runtime_over(&did, source.clone()).await;

    let first = me_profile(&runtime, &user_id).await;
    let second = me_profile(&runtime, &user_id).await;

    assert_eq!(first, None);
    assert_eq!(second, None);
    assert_eq!(
        source.fetch_count(),
        2,
        "a failed lookup leaves nothing to join"
    );
}

#[tokio::test(start_paused = true)]
async fn a_lookup_that_never_ends_is_dropped_at_its_cap() {
    let did = Did::from("did:plc:app-capped".to_string());
    let source = Arc::new(MemProfileSource::new(bounded_profile(&did)));
    source.never_answer();
    let (runtime, user_id) = runtime_over(&did, source.clone()).await;

    me_profile(&runtime, &user_id).await;
    tokio::time::sleep(LOOKUP_CAP).await;
    me_profile(&runtime, &user_id).await;

    assert_eq!(
        source.fetch_count(),
        2,
        "the capped lookup ended, so the next miss started its own"
    );
}

/// A [`ProfileSource`] whose every fetch panics, counting the attempts.
#[derive(Default)]
struct PanickingSource {
    fetches: AtomicUsize,
}

#[async_trait]
impl ProfileSource for PanickingSource {
    async fn fetch(&self, _did: &Did) -> anyhow::Result<Profile> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        panic!("the profile source panicked (fake)");
    }
}

#[tokio::test]
async fn a_lookup_whose_source_panics_is_fetched_again_on_the_next_miss() {
    let did = Did::from("did:plc:app-panicky".to_string());
    let source = Arc::new(PanickingSource::default());
    let (runtime, user_id) = runtime_over(&did, source.clone()).await;

    let first = me_profile(&runtime, &user_id).await;
    let second = me_profile(&runtime, &user_id).await;

    assert_eq!(first, None, "a panicked lookup reports no profile");
    assert_eq!(second, None);
    assert_eq!(
        source.fetches.load(Ordering::SeqCst),
        2,
        "a panicked lookup leaves nothing to join"
    );
}

#[test]
fn a_miss_on_a_closed_runtime_does_not_hang() {
    let did = Did::from("did:plc:app-closed".to_string());
    let source = Arc::new(MemProfileSource::new(bounded_profile(&did)));
    let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a runtime");
    let (runtime, _user_id) = tokio_runtime.block_on(runtime_over(&did, source));
    let handle = tokio_runtime.handle().clone();
    // Closes the task list: a spawn now drops its future in place.
    drop(tokio_runtime);

    let (polled, returned) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _entered = handle.enter();
        let lookup =
            runtime
                .profile_lookups
                .profile(&runtime.profile_cache, &runtime.profile_source, &did);
        let mut lookup = std::pin::pin!(lookup);
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let _ = lookup.as_mut().poll(&mut context);
        polled.send(()).ok();
    });
    let outcome = returned.recv_timeout(Duration::from_secs(5));

    assert!(
        !matches!(outcome, Err(std::sync::mpsc::RecvTimeoutError::Timeout)),
        "the miss deadlocked on the lookups' own mutex"
    );
}

/// The in-memory profile cache of a fresh fixture, for tests that drive
/// [`ProfileLookups`] directly.
fn empty_cache(did: &Did) -> Arc<dyn ProfileCache> {
    test_support::runtime::mem(did)
        .build()
        .runtime
        .profile_cache
}

#[tokio::test(start_paused = true)]
async fn clones_share_one_lookup() {
    let did = Did::from("did:plc:app-cloned".to_string());
    let source = Arc::new(MemProfileSource::new(bounded_profile(&did)));
    source.answer_after(PROFILE_WAIT / 2);
    let source_port: Arc<dyn ProfileSource> = source.clone();
    let cache = empty_cache(&did);
    let original = ProfileLookups::default();
    let clone = original.clone();

    let (first, second) = tokio::join!(
        original.profile(&cache, &source_port, &did),
        clone.profile(&cache, &source_port, &did)
    );

    assert_eq!(first, Some(bounded_profile(&did)));
    assert_eq!(second, Some(bounded_profile(&did)));
    assert_eq!(
        source.fetch_count(),
        1,
        "the clone joined the original's lookup"
    );
}

/// A [`ProfileSource`] answering each DID with its own profile after a delay,
/// counting the fetches.
#[derive(Default)]
struct PerDidSource {
    fetches: AtomicUsize,
}

impl PerDidSource {
    /// The profile this source answers for `did`.
    fn profile_of(did: &Did) -> Profile {
        Profile::new(
            did.clone(),
            format!("{}.bsky.social", did.as_ref().replace(':', "-")),
        )
    }
}

#[async_trait]
impl ProfileSource for PerDidSource {
    async fn fetch(&self, did: &Did) -> anyhow::Result<Profile> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(PROFILE_WAIT / 2).await;
        Ok(Self::profile_of(did))
    }
}

#[tokio::test(start_paused = true)]
async fn misses_for_two_dids_get_their_own_lookup_and_profile() {
    let alice = Did::from("did:plc:app-alice".to_string());
    let bob = Did::from("did:plc:app-bob".to_string());
    let source = Arc::new(PerDidSource::default());
    let source_port: Arc<dyn ProfileSource> = source.clone();
    let cache = empty_cache(&alice);
    let lookups = ProfileLookups::default();

    let (alices, bobs) = tokio::join!(
        lookups.profile(&cache, &source_port, &alice),
        lookups.profile(&cache, &source_port, &bob)
    );

    assert_eq!(alices, Some(PerDidSource::profile_of(&alice)));
    assert_eq!(bobs, Some(PerDidSource::profile_of(&bob)));
    assert_eq!(
        source.fetches.load(Ordering::SeqCst),
        2,
        "each DID got its own lookup"
    );
}

/// A [`ProfileCache`] that always misses and whose fill never returns.
struct StalledFillCache;

#[async_trait]
impl ProfileCache for StalledFillCache {
    async fn get(&self, _did: &Did) -> anyhow::Result<Option<Profile>> {
        Ok(None)
    }

    async fn put(&self, _profile: &Profile) -> anyhow::Result<()> {
        std::future::pending::<()>().await;
        Ok(())
    }
}

#[tokio::test(start_paused = true)]
async fn a_stalled_cache_fill_is_dropped_at_the_cap() {
    let did = Did::from("did:plc:app-stalled-fill".to_string());
    let source = Arc::new(MemProfileSource::new(bounded_profile(&did)));
    let source_port: Arc<dyn ProfileSource> = source.clone();
    let cache: Arc<dyn ProfileCache> = Arc::new(StalledFillCache);
    let lookups = ProfileLookups::default();

    lookups.profile(&cache, &source_port, &did).await;
    tokio::time::sleep(LOOKUP_CAP).await;
    lookups.profile(&cache, &source_port, &did).await;

    assert_eq!(
        source.fetch_count(),
        2,
        "the stalled fill ended at the cap, so the next miss fetched again"
    );
}
