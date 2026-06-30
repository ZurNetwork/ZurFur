//! [`ProfileCache`] over PostgreSQL: a TTL'd read-through cache of public PDS
//! profiles in the `profile_cache` table, so repeat views don't need the PDS
//! awake. Both the read (`get`) and the cache fill (`put`) are pool-backed and
//! `&self`. The cache write is a **documented exception** to the compile-enforced
//! Unit of Work (DD `24150017`): a read-through cache fill on the GET path carries
//! no transactional invariant, so it does not belong on the write-only
//! `UnitOfWork` handle — the same reasoning that exempts `session_store` and
//! `auth_store` (see the `no_bare_pool_writes` guard). See ZMVP-10 and
//! DESIGN/"Domains and Applications".

use chrono::{Duration, Utc};
use domain::{
    elements::{did::Did, profile::Profile},
    ports::ProfileCache,
};
use sqlx::{PgPool, query};

/// Postgres-backed read-through cache of public profiles (ZMVP-10). Freshness is
/// this adapter's policy: `get` treats an entry older than `ttl` as a miss, so a
/// stale profile is refetched from the PDS rather than served. `put` upserts, so a
/// refetch overwrites the prior copy in one round trip. Both run on the pool — the
/// cache write is a documented exception to the Unit of Work (see the module note).
pub struct PgProfileCache {
    pool: PgPool,
    ttl: Duration,
}

impl PgProfileCache {
    /// `ttl` is the freshness window: a cached entry older than this is treated as
    /// a miss. Taken as a `std::time::Duration` so the composition root needn't
    /// depend on chrono; converted once here.
    pub fn new(pool: PgPool, ttl: std::time::Duration) -> Self {
        let ttl = Duration::from_std(ttl).expect("profile cache TTL fits in chrono::Duration");
        Self { pool, ttl }
    }
}

#[async_trait::async_trait]
impl ProfileCache for PgProfileCache {
    /// The [`ttl`](PgProfileCache::new) is applied as a `fetched_at > cutoff`
    /// predicate, so a stale entry returns `None` (a cache miss) and the caller
    /// refetches — it is never served. The cutoff is computed app-side to keep
    /// the freshness window explicit and testable.
    async fn get(&self, did: &Did) -> anyhow::Result<Option<Profile>> {
        // Apply the TTL as a query predicate: an entry past it simply isn't
        // returned, so the caller sees a miss and refetches. Cutoff computed
        // app-side to keep the freshness window explicit and testable.
        let cutoff = Utc::now() - self.ttl;
        let row = query!(
            r#"
            SELECT
                did          AS "did!",
                handle       AS "handle!",
                display_name AS "display_name?",
                avatar_url   AS "avatar_url?"
            FROM profile_cache
            WHERE did = $1 AND fetched_at > $2
            "#,
            did.as_str(),
            cutoff,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| Profile {
            did: Did::new(row.did),
            handle: row.handle,
            display_name: row.display_name,
            avatar_url: row.avatar_url,
        }))
    }

    /// `INSERT ... ON CONFLICT (did) DO UPDATE`: a refetch overwrites the prior
    /// copy in one round trip and stamps `fetched_at = now()`, restarting the TTL
    /// window read by [`get`](PgProfileCache::get).
    ///
    /// GUARD EXCEPTION (DD `24150017`): this write runs on the pool, not a
    /// `UnitOfWork`. A best-effort read-through cache fill on the GET path is not a
    /// domain write — it has no transactional invariant to uphold, the caller
    /// swallows its failure, and routing it through a write transaction would make
    /// a read endpoint open one for nothing. So it is a documented exception to the
    /// bare-pool-write guard, alongside `session_store` and `auth_store`.
    async fn put(&self, profile: &Profile) -> anyhow::Result<()> {
        query!(
            r#"
            INSERT INTO profile_cache (did, handle, display_name, avatar_url, fetched_at)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (did) DO UPDATE SET
                handle       = EXCLUDED.handle,
                display_name = EXCLUDED.display_name,
                avatar_url   = EXCLUDED.avatar_url,
                fetched_at   = EXCLUDED.fetched_at
            "#,
            profile.did.as_str(),
            profile.handle,
            profile.display_name,
            profile.avatar_url,
            Utc::now(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
