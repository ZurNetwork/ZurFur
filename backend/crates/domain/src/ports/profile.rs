use async_trait::async_trait;

use crate::elements::did::Did;
use crate::elements::profile::Profile;

/// Reads a visitor's public profile from its source of truth, the user's PDS —
/// a public-boundary read: async, fallible, and possibly lagging.
#[async_trait]
pub trait ProfileSource: Send + Sync {
    /// Fetch the profile for a DID. The handle always resolves; `display_name`
    /// and `avatar_url` may be absent. Errors when the PDS is unreachable.
    async fn fetch(&self, did: &Did) -> anyhow::Result<Profile>;
}

/// A private-side read-through cache of public profiles, so repeat views don't
/// need the PDS awake. Pool-backed and `&self` — a documented exception to the
/// Unit of Work, since a cache fill carries no transactional invariant
///. Freshness policy lives in the implementation.
#[async_trait]
pub trait ProfileCache: Send + Sync {
    /// The cached profile for a DID, or `None` on a miss — absent and stale alike.
    /// The `Result` is for store errors, not misses.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<Profile>>;

    /// Store or refresh a profile, keyed by its DID. Idempotent; a best-effort
    /// fill on the read path, not a domain write.
    async fn put(&self, profile: &Profile) -> anyhow::Result<()>;
}
