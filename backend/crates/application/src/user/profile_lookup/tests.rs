use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use super::*;

/// How the scripted PDS answers a fetch.
#[derive(Clone)]
enum Answer {
    /// Never answers.
    Never,
    /// Fails at once.
    Fails,
    /// Answers with the profile after the delay.
    After(Duration, Profile),
}

/// A PDS whose answer the test sets, counting its fetches.
struct ScriptedPds {
    answer: Mutex<Answer>,
    fetches: AtomicUsize,
}

impl ScriptedPds {
    fn answering(answer: Answer) -> Arc<Self> {
        let pds = Self {
            answer: Mutex::new(answer),
            fetches: AtomicUsize::new(0),
        };
        Arc::new(pds)
    }

    fn answer_with(&self, answer: Answer) {
        *self.answer.lock().expect("the script's lock") = answer;
    }

    fn fetches(&self) -> usize {
        self.fetches.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl ProfileSource for ScriptedPds {
    async fn fetch(&self, _did: &Did) -> anyhow::Result<Profile> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        let answer = self.answer.lock().expect("the script's lock").clone();
        match answer {
            Answer::Never => std::future::pending().await,
            Answer::Fails => Err(anyhow::anyhow!("the PDS refused (test)")),
            Answer::After(delay, profile) => {
                tokio::time::sleep(delay).await;
                Ok(profile)
            }
        }
    }
}

/// An empty in-memory profile cache.
fn empty_cache() -> Arc<dyn ProfileCache> {
    adapter_mem::MemBackend::new().profile_cache()
}

fn carol() -> Did {
    Did::from("did:plc:profile-lookup-carol".to_owned())
}

fn carol_s_profile() -> Profile {
    Profile::new(carol(), "carol.test").with_display_name("Carol")
}

/// The Den's mode for `did`, and how long it took.
async fn timed_den_read(
    lookups: &ProfileLookups,
    cache: &Arc<dyn ProfileCache>,
    pds: &Arc<ScriptedPds>,
    did: &Did,
) -> (Option<Profile>, Duration) {
    let source: Arc<dyn ProfileSource> = pds.clone();
    let started = Instant::now();
    let profile = lookups.profile_unless_busy(cache, &source, did).await;
    (profile, started.elapsed())
}

/// `/me`'s mode for `did`, and how long it took.
async fn timed_me_read(
    lookups: &ProfileLookups,
    cache: &Arc<dyn ProfileCache>,
    pds: &Arc<ScriptedPds>,
    did: &Did,
) -> (Option<Profile>, Duration) {
    let source: Arc<dyn ProfileSource> = pds.clone();
    let started = Instant::now();
    let profile = lookups.profile(cache, &source, did).await;
    (profile, started.elapsed())
}

#[tokio::test(start_paused = true)]
async fn a_pds_that_never_answers_gives_up_after_the_wait() {
    let lookups = ProfileLookups::default();
    let cache = empty_cache();
    let pds = ScriptedPds::answering(Answer::Never);

    let (profile, waited) = timed_den_read(&lookups, &cache, &pds, &carol()).await;

    assert_eq!(profile, None);
    assert_eq!(waited, PROFILE_WAIT);
    assert_eq!(pds.fetches(), 1);
}

#[tokio::test(start_paused = true)]
async fn while_a_lookup_runs_every_later_read_falls_back_at_once_without_a_fetch() {
    let lookups = ProfileLookups::default();
    let cache = empty_cache();
    let pds = ScriptedPds::answering(Answer::Never);
    let (_, first_waited) = timed_den_read(&lookups, &cache, &pds, &carol()).await;

    let mut later_waits = Vec::new();
    for _ in 0..16 {
        let (profile, waited) = timed_den_read(&lookups, &cache, &pds, &carol()).await;
        assert_eq!(profile, None);
        later_waits.push(waited);
    }

    assert_eq!(first_waited, PROFILE_WAIT);
    assert_eq!(later_waits, vec![Duration::ZERO; 16]);
    assert_eq!(pds.fetches(), 1);
}

#[tokio::test(start_paused = true)]
async fn after_a_failure_reads_fall_back_at_once_for_the_hold_then_fetch_again() {
    let lookups = ProfileLookups::default();
    let cache = empty_cache();
    let pds = ScriptedPds::answering(Answer::Fails);
    let (failed, _) = timed_den_read(&lookups, &cache, &pds, &carol()).await;
    pds.answer_with(Answer::After(Duration::ZERO, carol_s_profile()));

    tokio::time::advance(FAILED_LOOKUP_HOLD - Duration::from_millis(1)).await;
    let (held, held_wait) = timed_den_read(&lookups, &cache, &pds, &carol()).await;
    let fetches_within_the_hold = pds.fetches();
    tokio::time::advance(Duration::from_millis(1)).await;
    let (after_the_hold, _) = timed_den_read(&lookups, &cache, &pds, &carol()).await;

    assert_eq!(failed, None);
    assert_eq!(held, None);
    assert_eq!(held_wait, Duration::ZERO);
    assert_eq!(fetches_within_the_hold, 1);
    assert_eq!(after_the_hold, Some(carol_s_profile()));
    assert_eq!(pds.fetches(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_lookup_that_runs_out_is_held_like_a_failure() {
    let lookups = ProfileLookups::default();
    let cache = empty_cache();
    let pds = ScriptedPds::answering(Answer::Never);
    timed_den_read(&lookups, &cache, &pds, &carol()).await;

    tokio::time::sleep(LOOKUP_CAP).await;
    let (held, held_wait) = timed_den_read(&lookups, &cache, &pds, &carol()).await;
    let fetches_within_the_hold = pds.fetches();
    tokio::time::sleep(FAILED_LOOKUP_HOLD).await;
    timed_den_read(&lookups, &cache, &pds, &carol()).await;

    assert_eq!(held, None);
    assert_eq!(held_wait, Duration::ZERO);
    assert_eq!(fetches_within_the_hold, 1);
    assert_eq!(pds.fetches(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_slow_pds_s_profile_fills_the_cache_for_a_later_read() {
    let lookups = ProfileLookups::default();
    let cache = empty_cache();
    let slow = PROFILE_WAIT + Duration::from_secs(3);
    let pds = ScriptedPds::answering(Answer::After(slow, carol_s_profile()));

    let (first, _) = timed_den_read(&lookups, &cache, &pds, &carol()).await;
    tokio::time::sleep(slow).await;
    let (later, later_wait) = timed_den_read(&lookups, &cache, &pds, &carol()).await;

    assert_eq!(first, None);
    assert_eq!(later, Some(carol_s_profile()));
    assert_eq!(later_wait, Duration::ZERO);
    assert_eq!(pds.fetches(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_clone_shares_the_running_lookups() {
    let lookups = ProfileLookups::default();
    let cache = empty_cache();
    let pds = ScriptedPds::answering(Answer::Never);
    timed_den_read(&lookups, &cache, &pds, &carol()).await;

    let (profile, waited) = timed_den_read(&lookups.clone(), &cache, &pds, &carol()).await;

    assert_eq!(profile, None);
    assert_eq!(waited, Duration::ZERO);
    assert_eq!(pds.fetches(), 1);
}

#[tokio::test(start_paused = true)]
async fn me_s_mode_still_joins_a_running_lookup_and_waits_for_it() {
    let lookups = ProfileLookups::default();
    let cache = empty_cache();
    let pds = ScriptedPds::answering(Answer::Never);
    timed_den_read(&lookups, &cache, &pds, &carol()).await;

    let (profile, waited) = timed_me_read(&lookups, &cache, &pds, &carol()).await;

    assert_eq!(profile, None);
    assert_eq!(waited, PROFILE_WAIT);
    assert_eq!(pds.fetches(), 1);
}

#[tokio::test(start_paused = true)]
async fn me_s_mode_ignores_the_hold_and_fetches_again_at_once() {
    let lookups = ProfileLookups::default();
    let cache = empty_cache();
    let pds = ScriptedPds::answering(Answer::Fails);
    timed_den_read(&lookups, &cache, &pds, &carol()).await;
    pds.answer_with(Answer::After(Duration::ZERO, carol_s_profile()));

    let (profile, _) = timed_me_read(&lookups, &cache, &pds, &carol()).await;

    assert_eq!(profile, Some(carol_s_profile()));
    assert_eq!(pds.fetches(), 2);
}

#[tokio::test(start_paused = true)]
async fn the_failure_record_drops_holds_that_ran_out() {
    let lookups = ProfileLookups::default();
    let cache = empty_cache();
    let pds = ScriptedPds::answering(Answer::Fails);
    let dave = Did::from("did:plc:profile-lookup-dave".to_owned());
    timed_den_read(&lookups, &cache, &pds, &carol()).await;

    tokio::time::advance(FAILED_LOOKUP_HOLD).await;
    timed_den_read(&lookups, &cache, &pds, &dave).await;

    let held: Vec<Did> = lookups
        .failures
        .failed_at
        .lock()
        .expect("the record's lock")
        .keys()
        .cloned()
        .collect();
    assert_eq!(held, [dave]);
}

#[tokio::test(start_paused = true)]
async fn a_success_releases_the_hold() {
    let lookups = ProfileLookups::default();
    let cache = empty_cache();
    let pds = ScriptedPds::answering(Answer::Fails);
    timed_den_read(&lookups, &cache, &pds, &carol()).await;
    pds.answer_with(Answer::After(Duration::ZERO, carol_s_profile()));

    timed_me_read(&lookups, &cache, &pds, &carol()).await;

    assert!(!lookups.failures.holds(&carol()));
}
