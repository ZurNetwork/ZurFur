//! Round-trips the `plc_operations` append-only log against a throwaway PostgreSQL
//! container: appends chain in submission order, `latest_cid` returns the most recent
//! per DID, and the unique `cid` index rejects a duplicate. Requires a container
//! runtime socket (DOCKER_HOST honored).
use adapter_pg::{PgPlcOperationLog, PgPool};
use domain::{
    elements::{did::Did, plc_operation::PlcOperationRecord},
    ports::PlcOperationLog,
};

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
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

// A handle-update op (ZMVP-50: `plc_operation` with a non-null `prev`) round-trips:
// appended after the genesis, it becomes the `latest_cid` the NEXT op must chain onto —
// and replaying the identical update (same content → same cid) is rejected by the
// unique index, the DB half of the update's idempotency contract.
#[tokio::test]
async fn an_update_op_becomes_the_latest_and_a_replay_is_rejected() {
    let (pool, _container) = fresh_pool().await;
    let log = PgPlcOperationLog::new(pool.clone());
    let did = Did::new("did:plc:oplog-update".to_string());

    log.append(&record(&did, "bafyreigenesis2", "plc_operation", None))
        .await
        .expect("append genesis");
    log.append(&record(
        &did,
        "bafyreiupdate",
        "plc_operation",
        Some("bafyreigenesis2"),
    ))
    .await
    .expect("append update");

    assert_eq!(
        log.latest_cid(&did).await.expect("latest_cid").as_deref(),
        Some("bafyreiupdate"),
        "the update is now the DID's latest op",
    );

    assert!(
        log.append(&record(
            &did,
            "bafyreiupdate",
            "plc_operation",
            Some("bafyreigenesis2"),
        ))
        .await
        .is_err(),
        "an identical replay (same content, same cid) is rejected by the unique index",
    );
}

// NO CHAIN FORK (ZMVP-50 F1): a given `prev` may be chained onto at most once, so two
// DIFFERENT ops (distinct cids, so `UNIQUE(cid)` does NOT catch them) chaining the same
// prev cannot both land — the partial `UNIQUE(did, prev)` index rejects the second. This
// is what stops concurrent handle updates forking the DID's local chain.
#[tokio::test]
async fn two_different_ops_cannot_chain_the_same_prev() {
    let (pool, _container) = fresh_pool().await;
    let log = PgPlcOperationLog::new(pool.clone());
    let did = Did::new("did:plc:oplog-fork".to_string());

    log.append(&record(&did, "bafyreigenesis3", "plc_operation", None))
        .await
        .expect("append genesis");
    log.append(&record(
        &did,
        "bafyreiupdatex",
        "plc_operation",
        Some("bafyreigenesis3"),
    ))
    .await
    .expect("first update chains onto genesis");

    assert!(
        log.append(&record(
            &did,
            "bafyreiupdatey",
            "plc_operation",
            Some("bafyreigenesis3"),
        ))
        .await
        .is_err(),
        "a SECOND, different op chaining the same prev is rejected — the chain cannot fork",
    );
}

// `latest_op` returns the DID's most recent op in full (cid, type, prev, JSON) so an
// update can carry the prior op's public fields forward without decrypting custody keys
// (ZMVP-50 F2).
#[tokio::test]
async fn latest_op_returns_the_full_most_recent_record() {
    let (pool, _container) = fresh_pool().await;
    let log = PgPlcOperationLog::new(pool.clone());
    let did = Did::new("did:plc:oplog-latestop".to_string());

    assert!(
        log.latest_op(&did).await.expect("latest_op").is_none(),
        "no operations logged yet",
    );

    log.append(&record(&did, "bafyreigenesis4", "plc_operation", None))
        .await
        .expect("append genesis");
    log.append(&record(
        &did,
        "bafyreiupdatez",
        "plc_operation",
        Some("bafyreigenesis4"),
    ))
    .await
    .expect("append update");

    let latest = log
        .latest_op(&did)
        .await
        .expect("latest_op")
        .expect("an op is logged");
    assert_eq!(latest.cid, "bafyreiupdatez", "the most recent op");
    assert_eq!(latest.op_type, "plc_operation");
    assert_eq!(latest.prev.as_deref(), Some("bafyreigenesis4"));
    let json: serde_json::Value =
        serde_json::from_str(&latest.operation_json).expect("operation_json is valid JSON");
    assert_eq!(
        json["type"], "plc_operation",
        "the stored op body round-trips"
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
