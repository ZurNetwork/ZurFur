//! [`ProfileCache`] over PostgreSQL: a TTL'd read-through cache of public PDS
//! profiles in `profile_cache`, so repeat views don't need the PDS awake. Both
//! `get` and `put` are pool-backed; `put` is a documented exception to the
//! compile-enforced Unit of Work (DD 24150017) — a cache fill on the GET path
//! carries no transactional invariant (see `no_bare_pool_writes`).

use chrono::{Duration, Utc};
use domain::{
    elements::{did::Did, profile::Profile},
    ports::ProfileCache,
};
use sqlx::PgPool;

use crate::queries::profile as sql;

/// Postgres-backed read-through cache of public profiles. `get` treats an
/// entry older than `ttl` as a miss; `put` upserts, so a refetch overwrites
/// the prior copy in one round trip.
pub struct PgProfileCache {
    pool: PgPool,
    ttl: Duration,
}

impl PgProfileCache {
    /// `ttl` is the freshness window: a cached entry older than this is a miss.
    /// Taken as `std::time::Duration` so the composition root needn't depend on chrono.
    pub fn new(pool: PgPool, ttl: std::time::Duration) -> Self {
        let ttl = Duration::from_std(ttl).expect("profile cache TTL fits in chrono::Duration");
        Self { pool, ttl }
    }
}

#[async_trait::async_trait]
impl ProfileCache for PgProfileCache {
    /// The [`ttl`](PgProfileCache::new) is applied as a `fetched_at > cutoff`
    /// predicate, so a stale entry returns `None` (a miss) and the caller refetches.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<Profile>> {
        let cutoff = Utc::now() - self.ttl;
        let row = sql::get(&self.pool, did.as_str(), cutoff).await?;

        Ok(row.map(|row| Profile {
            did: Did::new(row.did),
            handle: row.handle,
            display_name: row.display_name,
            avatar_url: row.avatar_url,
        }))
    }

    /// Upserts: a refetch overwrites the prior copy and stamps `fetched_at =
    /// now()`, restarting the TTL window [`get`](PgProfileCache::get) reads.
    /// Runs on the pool, not a `UnitOfWork` — a documented exception (DD 24150017):
    /// a best-effort cache fill on the GET path has no transactional invariant.
    async fn put(&self, profile: &Profile) -> anyhow::Result<()> {
        sql::put(
            &self.pool,
            profile.did.as_str(),
            &profile.handle,
            profile.display_name.as_deref(),
            profile.avatar_url.as_deref(),
            Utc::now(),
        )
        .await?;
        Ok(())
    }
}
