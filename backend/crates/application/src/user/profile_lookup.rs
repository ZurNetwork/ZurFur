//! The profile read-through with a bounded wait: a cache miss starts one
//! background lookup per DID, which every concurrent miss joins and which runs
//! to its end whether or not anyone still waits. The Den's mode joins nothing:
//! a miss while a lookup runs, or soon after one failed or did not fill the
//! cache, answers at once.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use domain::elements::{did::Did, profile::Profile};
use domain::ports::{ProfileCache, ProfileSource};
use tokio::sync::watch;
use tokio::time::Instant;

/// How long a caller waits for a profile the cache did not have.
pub const PROFILE_WAIT: Duration = Duration::from_secs(2);

/// The longest one background lookup runs, the fetch and the cache fill
/// together.
pub const LOOKUP_CAP: Duration = Duration::from_secs(30);

/// How long after a lookup failed, ran out or did not fill the cache
/// [`ProfileLookups::profile_unless_busy`] answers `None` at once rather than
/// look again.
pub const FAILED_LOOKUP_HOLD: Duration = Duration::from_secs(30);

/// The running lookups, keyed by the DID each one reads.
type Running = Arc<Mutex<HashMap<Did, watch::Receiver<Lookup>>>>;

/// Where one lookup stands, as its joiners see it.
enum Lookup {
    /// The fetch or the cache fill is still under way.
    Running,
    /// The lookup ended: the profile, or `None` when it failed or ran out.
    Ended(Option<Profile>),
}

impl Lookup {
    fn has_ended(&self) -> bool {
        matches!(self, Lookup::Ended(_))
    }

    fn profile(&self) -> Option<Profile> {
        match self {
            Lookup::Running | Lookup::Ended(None) => None,
            Lookup::Ended(Some(profile)) => Some(profile.clone()),
        }
    }
}

/// The profile lookups now running, at most one per DID. Built once for the
/// process; every clone shares the same set.
#[derive(Clone, Default)]
pub struct ProfileLookups {
    running: Running,
    failures: Failures,
}

impl ProfileLookups {
    /// `did`'s profile from `cache`, else from the lookup for `did` (joined if
    /// one is running, started otherwise), waited on for at most
    /// [`PROFILE_WAIT`]. `None` when the wait runs out or the lookup fails.
    pub async fn profile(
        &self,
        cache: &Arc<dyn ProfileCache>,
        source: &Arc<dyn ProfileSource>,
        did: &Did,
    ) -> Option<Profile> {
        if let Ok(Some(profile)) = cache.get(did).await {
            return Some(profile);
        }
        let lookup = self.join_or_start(cache, source, did);
        wait_for_end(lookup).await
    }

    /// `did`'s profile from `cache`, else from a lookup for `did` started here
    /// and waited on for at most [`PROFILE_WAIT`]. `None` at once, starting
    /// nothing, while a lookup for `did` is running or within
    /// [`FAILED_LOOKUP_HOLD`] of one failing or not filling the cache.
    pub async fn profile_unless_busy(
        &self,
        cache: &Arc<dyn ProfileCache>,
        source: &Arc<dyn ProfileSource>,
        did: &Did,
    ) -> Option<Profile> {
        if let Ok(Some(profile)) = cache.get(did).await {
            return Some(profile);
        }
        let lookup = self.start_unless_busy(cache, source, did)?;
        wait_for_end(lookup).await
    }

    /// The running lookup for `did`, or a new one spawned here.
    fn join_or_start(
        &self,
        cache: &Arc<dyn ProfileCache>,
        source: &Arc<dyn ProfileSource>,
        did: &Did,
    ) -> watch::Receiver<Lookup> {
        let ended = {
            let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(lookup) = running.get(did) {
                return lookup.clone();
            }
            register(&mut running, did)
        };
        self.spawn_lookup(cache, source, did, ended)
    }

    /// A new lookup for `did`, spawned here, unless one is running or `did` is
    /// held.
    fn start_unless_busy(
        &self,
        cache: &Arc<dyn ProfileCache>,
        source: &Arc<dyn ProfileSource>,
        did: &Did,
    ) -> Option<watch::Receiver<Lookup>> {
        let ended = {
            let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
            if running.contains_key(did) || self.failures.holds(did) {
                return None;
            }
            register(&mut running, did)
        };
        Some(self.spawn_lookup(cache, source, did, ended))
    }

    /// Spawn the lookup `register` entered for `did`, and hand back its
    /// receiver. Called with the lock released: a runtime shutting down drops
    /// the task, and so its entry, right here, and the entry's drop takes the lock.
    fn spawn_lookup(
        &self,
        cache: &Arc<dyn ProfileCache>,
        source: &Arc<dyn ProfileSource>,
        did: &Did,
        ended: watch::Sender<Lookup>,
    ) -> watch::Receiver<Lookup> {
        let lookup = ended.subscribe();
        let entry = RunningEntry {
            running: self.running.clone(),
            did: did.clone(),
        };
        tokio::spawn(run_lookup(
            cache.clone(),
            source.clone(),
            entry,
            ended,
            self.failures.clone(),
        ));
        lookup
    }
}

/// Enter a new running lookup for `did` and hand back its sender.
fn register(
    running: &mut HashMap<Did, watch::Receiver<Lookup>>,
    did: &Did,
) -> watch::Sender<Lookup> {
    let (ended, lookup) = watch::channel(Lookup::Running);
    running.insert(did.clone(), lookup);
    ended
}

/// The profile `lookup` ends with, waited on for at most [`PROFILE_WAIT`].
async fn wait_for_end(mut lookup: watch::Receiver<Lookup>) -> Option<Profile> {
    let waited = tokio::time::timeout(PROFILE_WAIT, lookup.wait_for(Lookup::has_ended)).await;
    let Ok(Ok(ended)) = waited else {
        return None;
    };
    ended.profile()
}

/// What one lookup brought back.
enum Fetched {
    /// The profile, now in the cache.
    Filled(Profile),
    /// The profile, which the cache did not store.
    Unfilled(Profile),
    /// Nothing: the fetch failed or ran out.
    Nothing,
}

impl Fetched {
    fn into_profile(self) -> Option<Profile> {
        match self {
            Fetched::Filled(profile) | Fetched::Unfilled(profile) => Some(profile),
            Fetched::Nothing => None,
        }
    }
}

/// Fetch and cache the profile within [`LOOKUP_CAP`], hold or release `did`,
/// retire the entry, then tell the joiners how it ended.
async fn run_lookup(
    cache: Arc<dyn ProfileCache>,
    source: Arc<dyn ProfileSource>,
    entry: RunningEntry,
    ended: watch::Sender<Lookup>,
    failures: Failures,
) {
    let fetched = tokio::time::timeout(LOOKUP_CAP, fetch_and_fill(&*cache, &*source, &entry.did))
        .await
        .unwrap_or_else(|_| {
            tracing::info!("a profile lookup ran out of time");
            Fetched::Nothing
        });
    // Settled before the entry retires, so no read between the two looks again.
    match fetched {
        Fetched::Filled(_) => failures.release(&entry.did),
        Fetched::Unfilled(_) | Fetched::Nothing => failures.hold(&entry.did),
    }
    // Retired before the joiners hear, so a miss after them starts afresh.
    drop(entry);
    ended.send_replace(Lookup::Ended(fetched.into_profile()));
}

/// `did`'s profile from `source`, stored in `cache` on the way. The fill is
/// pool-backed and best effort: a `put` failure never loses the profile.
async fn fetch_and_fill(
    cache: &dyn ProfileCache,
    source: &dyn ProfileSource,
    did: &Did,
) -> Fetched {
    let Ok(profile) = source.fetch(did).await else {
        return Fetched::Nothing;
    };
    match cache.put(&profile).await {
        Ok(()) => Fetched::Filled(profile),
        Err(_) => Fetched::Unfilled(profile),
    }
}

/// One running lookup's entry, removed when the lookup ends by any path: an
/// answer, a failure, the cap, a panic, or the runtime shutting down.
struct RunningEntry {
    running: Running,
    did: Did,
}

impl Drop for RunningEntry {
    fn drop(&mut self) {
        self.running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.did);
    }
}

/// When each DID's last lookup failed, ran out or did not fill the cache, kept
/// only for [`FAILED_LOOKUP_HOLD`]. Every clone shares the same record.
#[derive(Clone, Default)]
struct Failures {
    failed_at: Arc<Mutex<HashMap<Did, Instant>>>,
}

impl Failures {
    /// Hold `did` from now.
    fn hold(&self, did: &Did) {
        let now = Instant::now();
        self.pruned(now).insert(did.clone(), now);
    }

    /// Release `did`'s hold, if any.
    fn release(&self, did: &Did) {
        self.pruned(Instant::now()).remove(did);
    }

    /// The record, locked, with the holds that ran out by `now` dropped, so it
    /// grows only with the DIDs held within one hold.
    fn pruned(&self, now: Instant) -> MutexGuard<'_, HashMap<Did, Instant>> {
        let mut failed_at = self
            .failed_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        failed_at.retain(|_, at| now.duration_since(*at) < FAILED_LOOKUP_HOLD);
        failed_at
    }

    /// Whether `did` was held less than [`FAILED_LOOKUP_HOLD`] ago.
    fn holds(&self, did: &Did) -> bool {
        self.failed_at
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(did)
            .is_some_and(|at| at.elapsed() < FAILED_LOOKUP_HOLD)
    }
}

#[cfg(test)]
mod tests;
