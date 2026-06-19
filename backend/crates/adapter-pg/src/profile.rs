use chrono::{Duration, Utc};
use domain::{
    elements::{did::Did, profile::Profile},
    ports::ProfileCache,
};
use sqlx::{PgPool, query};

/// Postgres-backed read-through cache of public profiles (ZMVP-10). Freshness is
/// this adapter's policy: `get` treats an entry older than `ttl` as a miss, so a
/// stale profile is refetched from the PDS rather than served. `put` upserts, so
/// a refetch overwrites the prior copy in one round trip.
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
