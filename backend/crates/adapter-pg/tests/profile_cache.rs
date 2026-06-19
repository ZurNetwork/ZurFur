//! Round-trips `PgProfileCache` against a throwaway PostgreSQL container, proving
//! the migration-created `profile_cache` table stores and reads profiles, that the
//! TTL predicate hides stale entries, and that `put` upserts. Requires a container
//! runtime socket (DOCKER_HOST honored).
use std::time::Duration;

use adapter_pg::{PgPool, PgProfileCache};
use domain::{
    elements::{did::Did, profile::Profile},
    ports::ProfileCache,
};
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};

/// Boots a fresh database and runs migrations. The container is returned so the
/// caller keeps it alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    let container = Postgres::default()
        .start()
        .await
        .expect("postgres container should start");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped postgres port");
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = adapter_pg::connect(&database_url)
        .await
        .expect("pool connects");
    adapter_pg::migrate(&pool).await.expect("migrations run");
    (pool, container)
}

fn profile(did: &str) -> Profile {
    Profile {
        did: Did::new(did.to_string()),
        handle: "alice.bsky.social".to_string(),
        display_name: Some("Alice".to_string()),
        avatar_url: Some("https://pds.example/avatar/alice.jpg".to_string()),
    }
}

#[tokio::test]
async fn put_then_get_round_trips_within_ttl() {
    let (pool, _container) = fresh_pool().await;
    let cache = PgProfileCache::new(pool, Duration::from_secs(3600));
    let p = profile("did:plc:alice");

    cache.put(&p).await.expect("put");
    let got = cache.get(&p.did).await.expect("get");

    assert_eq!(got, Some(p), "a fresh entry round-trips intact");
}

#[tokio::test]
async fn get_unknown_did_is_a_miss() {
    let (pool, _container) = fresh_pool().await;
    let cache = PgProfileCache::new(pool, Duration::from_secs(3600));

    let got = cache
        .get(&Did::new("did:plc:nobody".to_string()))
        .await
        .expect("get");

    assert_eq!(got, None);
}

#[tokio::test]
async fn entries_past_the_ttl_read_as_a_miss() {
    let (pool, _container) = fresh_pool().await;
    // Zero TTL: the row is stale the instant after it's written, so the freshness
    // predicate must hide it — proving the TTL is enforced, with no sleep needed.
    let cache = PgProfileCache::new(pool, Duration::ZERO);
    let p = profile("did:plc:alice");

    cache.put(&p).await.expect("put");
    let got = cache.get(&p.did).await.expect("get");

    assert_eq!(got, None, "an entry older than the TTL must read as a miss");
}

#[tokio::test]
async fn put_upserts_the_latest_profile() {
    let (pool, _container) = fresh_pool().await;
    let cache = PgProfileCache::new(pool, Duration::from_secs(3600));
    let did = "did:plc:alice";

    cache.put(&profile(did)).await.expect("first put");
    let mut updated = profile(did);
    updated.display_name = Some("Alice Renamed".to_string());
    updated.avatar_url = None;
    cache.put(&updated).await.expect("second put");

    let got = cache.get(&Did::new(did.to_string())).await.expect("get");
    assert_eq!(
        got,
        Some(updated),
        "a second put overwrites the first (upsert keyed by did)"
    );
}
