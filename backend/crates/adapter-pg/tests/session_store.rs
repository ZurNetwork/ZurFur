//! Round-trips `PgSessionStore` against a throwaway PostgreSQL container, proving
//! the migration-created `tower_sessions.session` table actually persists sessions.
//! Requires a container runtime socket (DOCKER_HOST honored).
use std::collections::HashMap;

use adapter_pg::PgSessionStore;
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};
use time::{Duration, OffsetDateTime};
use tower_sessions_core::{
    SessionStore,
    session::{Id, Record},
    session_store::ExpiredDeletion,
};

/// Boots a fresh database, runs migrations, and returns the store. The container
/// is returned too so the caller keeps it alive for the test's duration.
async fn fresh_store() -> (PgSessionStore, impl Sized) {
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
    (PgSessionStore::new(pool), container)
}

fn record(did: &str, expiry_date: OffsetDateTime) -> Record {
    let mut data = HashMap::new();
    data.insert("did".to_string(), serde_json::json!(did));
    Record {
        id: Id::default(),
        data,
        expiry_date,
    }
}

#[tokio::test]
async fn create_load_save_delete_roundtrip() {
    let (store, _container) = fresh_store().await;

    let mut rec = record(
        "did:plc:abc123",
        OffsetDateTime::now_utc() + Duration::hours(1),
    );
    store.create(&mut rec).await.expect("create");

    let loaded = store
        .load(&rec.id)
        .await
        .expect("load")
        .expect("session present after create");
    assert_eq!(loaded.data["did"], serde_json::json!("did:plc:abc123"));

    // save overwrites in place
    let mut updated = loaded;
    updated
        .data
        .insert("did".to_string(), serde_json::json!("did:plc:xyz789"));
    store.save(&updated).await.expect("save");
    let reloaded = store
        .load(&rec.id)
        .await
        .expect("load")
        .expect("session present after save");
    assert_eq!(reloaded.data["did"], serde_json::json!("did:plc:xyz789"));

    // delete removes it
    store.delete(&rec.id).await.expect("delete");
    assert!(
        store.load(&rec.id).await.expect("load").is_none(),
        "session should be gone after delete"
    );
}

#[tokio::test]
async fn expired_sessions_are_not_loaded_then_swept() {
    let (store, _container) = fresh_store().await;

    let mut rec = record(
        "did:plc:stale",
        OffsetDateTime::now_utc() - Duration::minutes(1),
    );
    store.create(&mut rec).await.expect("create");

    // An expired row is never handed back by load, even though it still exists.
    assert!(
        store.load(&rec.id).await.expect("load").is_none(),
        "expired session must not load"
    );

    // ExpiredDeletion sweeps it without error; a still-valid session survives.
    let mut live = record(
        "did:plc:fresh",
        OffsetDateTime::now_utc() + Duration::hours(1),
    );
    store.create(&mut live).await.expect("create live");
    store.delete_expired().await.expect("delete_expired");
    assert!(
        store.load(&live.id).await.expect("load").is_some(),
        "unexpired session must survive the sweep"
    );
}
