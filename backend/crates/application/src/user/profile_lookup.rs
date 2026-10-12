//! The profile read-through with a bounded wait: a cache miss starts one
//! background lookup per DID, which every concurrent miss joins and which runs
//! to its end whether or not anyone still waits.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use domain::elements::{did::Did, profile::Profile};
use domain::ports::{ProfileCache, ProfileSource};
use tokio::sync::watch;

/// How long a caller waits for a profile the cache did not have.
pub const PROFILE_WAIT: Duration = Duration::from_secs(2);

/// The longest one background lookup runs, the fetch and the cache fill
/// together.
pub const LOOKUP_CAP: Duration = Duration::from_secs(30);

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
        let mut lookup = self.join_or_start(cache, source, did);
        let waited = tokio::time::timeout(PROFILE_WAIT, lookup.wait_for(Lookup::has_ended)).await;
        let Ok(Ok(ended)) = waited else {
            return None;
        };
        ended.profile()
    }

    /// The running lookup for `did`, or a new one spawned here.
    fn join_or_start(
        &self,
        cache: &Arc<dyn ProfileCache>,
        source: &Arc<dyn ProfileSource>,
        did: &Did,
    ) -> watch::Receiver<Lookup> {
        let (ended, lookup) = {
            let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(lookup) = running.get(did) {
                return lookup.clone();
            }
            let (ended, lookup) = watch::channel(Lookup::Running);
            running.insert(did.clone(), lookup.clone());
            (ended, lookup)
        };
        // Spawned with the lock released: a runtime shutting down drops the
        // task, and so its entry, right here, and the entry's drop takes the lock.
        let entry = RunningEntry {
            running: self.running.clone(),
            did: did.clone(),
        };
        tokio::spawn(run_lookup(cache.clone(), source.clone(), entry, ended));
        lookup
    }
}

/// Fetch and cache the profile within [`LOOKUP_CAP`], retire the entry, then
/// tell the joiners how it ended.
async fn run_lookup(
    cache: Arc<dyn ProfileCache>,
    source: Arc<dyn ProfileSource>,
    entry: RunningEntry,
    ended: watch::Sender<Lookup>,
) {
    let fetched = tokio::time::timeout(LOOKUP_CAP, fetch_and_fill(&*cache, &*source, &entry.did))
        .await
        .unwrap_or_else(|_| {
            tracing::info!("a profile lookup ran out of time");
            None
        });
    // Retired before the joiners hear, so a miss after them starts afresh.
    drop(entry);
    ended.send_replace(Lookup::Ended(fetched));
}

/// `did`'s profile from `source`, stored in `cache` on the way. The fill is
/// pool-backed and best effort: a `put` failure never loses the profile.
async fn fetch_and_fill(
    cache: &dyn ProfileCache,
    source: &dyn ProfileSource,
    did: &Did,
) -> Option<Profile> {
    let profile = source.fetch(did).await.ok()?;
    let _ = cache.put(&profile).await;
    Some(profile)
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
