//! Round-trips the `plc_operations` append-only log against a throwaway PostgreSQL
//! container: appends chain in submission order, `latest_cid` returns the most recent
//! per DID, and the unique `cid` index rejects a duplicate. Requires a container
//! runtime socket (DOCKER_HOST honored).
use adapter_pg::{PgPlcOperationLog, PgPool};
use domain::{
    elements::{did::Did, plc_operation::PlcOperationRecord},
    ports::PlcOperationLog,
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

fn record(did: &Did, cid: &str, op_type: &str, prev: Option<&str>) -> PlcOperationRecord {
    PlcOperationRecord {
        did: did.clone(),
        cid: cid.to_string(),
        op_type: op_type.to_string(),
        prev: prev.map(str::to_string),
        operation_json: format!(r#"{{"type":"{op_type}"}}"#),
    }
}

// The log chains in submission order: after a genesis then a tombstone, `latest_cid`
// returns the tombstone's CID (the DID's most recent op). Empty for an unknown DID, and
// scoped per DID.
#[tokio::test]
async fn append_then_latest_cid_returns_the_most_recent_per_did() {
    let (pool, _container) = fresh_pool().await;
    let log = PgPlcOperationLog::new(pool.clone());
    let did = Did::new("did:plc:oplog-a".to_string());

    assert!(
        log.latest_cid(&did).await.expect("latest_cid").is_none(),
        "no operations logged yet"
    );

    log.append(&record(&did, "bafyreigenesis", "plc_operation", None))
        .await
        .expect("append genesis");
    assert_eq!(
        log.latest_cid(&did).await.expect("latest_cid").as_deref(),
        Some("bafyreigenesis"),
    );

    log.append(&record(
        &did,
        "bafyreitombstone",
        "plc_tombstone",
        Some("bafyreigenesis"),
    ))
    .await
    .expect("append tombstone");
    assert_eq!(
        log.latest_cid(&did).await.expect("latest_cid").as_deref(),
        Some("bafyreitombstone"),
        "the latest op is now the tombstone",
    );

    // Another DID's log is independent.
    let other = Did::new("did:plc:oplog-b".to_string());
    assert!(
        log.latest_cid(&other).await.expect("latest_cid").is_none(),
        "a different DID has no operations",
    );
}

// The `cid` unique index rejects a duplicate append — a content-addressed op is logged
// at most once.
#[tokio::test]
async fn a_duplicate_cid_is_rejected() {
    let (pool, _container) = fresh_pool().await;
    let log = PgPlcOperationLog::new(pool.clone());
    let did = Did::new("did:plc:oplog-dup".to_string());

    log.append(&record(&did, "bafyreidup", "plc_operation", None))
        .await
        .expect("first append");
    assert!(
        log.append(&record(&did, "bafyreidup", "plc_tombstone", None))
            .await
            .is_err(),
        "the unique cid index rejects a duplicate",
    );
}
