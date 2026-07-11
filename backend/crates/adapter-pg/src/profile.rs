//! [`ProfileCache`] over PostgreSQL: a TTL'd read-through cache of public PDS
//! profiles in the `profile_cache` table, so repeat views don't need the PDS
//! awake. Both the read (`get`) and the cache fill (`put`) are pool-backed and
//! `&self`. The cache write is a **documented exception** to the compile-enforced
//! Unit of Work (DD `24150017`): a read-through cache fill on the GET path carries
//! no transactional invariant, so it does not belong on the write-only
//! `UnitOfWork` handle — the same reasoning that exempts `session_store` and
//! `auth_store` (see the `no_bare_pool_writes` guard). See ZMVP-10 and
//! DESIGN/"Domains and Applications".
//!
//! The SQL lives in `queries/profile/`, embedded via `include_str!` and verified
//! by the `query_files_prepare` test.

use crate::queries::ProfileQuery;
use chrono::{Duration, Utc};
use domain::{
    elements::{did::Did, profile::Profile},
    ports::ProfileCache,
};
use sqlx::PgPool;

/// A `profile_cache` row; the domain [`Profile`] is rebuilt from it via [`From`].
#[derive(sqlx::FromRow)]
struct ProfileRow {
    did: String,
    handle: String,
    display_name: Option<String>,
    avatar_url: Option<String>,
}

impl From<ProfileRow> for Profile {
    fn from(row: ProfileRow) -> Self {
        Profile {
            did: Did::new(row.did),
            handle: row.handle,
            display_name: row.display_name,
            avatar_url: row.avatar_url,
        }
    }
}

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
        let row: Option<ProfileRow> = sqlx::query_as(ProfileQuery::Get.sql())
            .bind(did.as_str())
            .bind(cutoff)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(Into::into))
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
        sqlx::query(ProfileQuery::Put.sql())
            .bind(profile.did.as_str())
            .bind(&profile.handle)
            .bind(&profile.display_name)
            .bind(&profile.avatar_url)
            .bind(Utc::now())
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
