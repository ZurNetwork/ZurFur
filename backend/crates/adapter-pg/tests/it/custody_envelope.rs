//! The custody envelope column: seed `account_keys` on the schema just before
//! it, run it and nothing after it, and check the kept 0.1 ciphertext, the new
//! column and the rule that every row holds one of the two; then the custody
//! queries on either side of it. Requires a container runtime socket
//! (DOCKER_HOST honored).

use adapter_pg::{PgPool, queries::key_store};

use crate::migration_steps::{bare_pool, run_migrations_before, run_migrations_through};

/// The envelope's migration, as sqlx numbers it.
const CUSTODY_ENVELOPE: i64 = 20261022231354;

/// The constraint that refuses a row holding neither blob.
const HOLDS_KEYS: &str = "account_keys_holds_keys";

/// Two 0.1 rows, out of DID order, their ciphertexts carrying zero bytes and
/// every byte value so a lossy copy shows.
fn legacy_rows() -> [(&'static str, Vec<u8>); 2] {
    let every_byte: Vec<u8> = (0..=255u8).collect();
    let with_zeros = vec![0u8, 1, 0, 0, 255, 0];
    [
        ("did:plc:custodyzulu", every_byte),
        ("did:plc:custodyalpha", with_zeros),
    ]
}

/// Store one 0.1 row, naming only the columns the table had before the envelope.
async fn seed_legacy(pool: &PgPool, did: &str, wrapped_keys: &[u8]) {
    sqlx::query(
        "INSERT INTO account_keys (did, wrapped_keys, created_at)
         VALUES ($1, $2, now())",
    )
    .bind(did)
    .bind(wrapped_keys)
    .execute(pool)
    .await
    .expect("seed a 0.1 row");
}

/// Store one row with the two blobs given, on the schema after the envelope.
async fn try_insert(
    pool: &PgPool,
    did: &str,
    wrapped_keys: Option<&[u8]>,
    sealed_keys: Option<&[u8]>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO account_keys (did, wrapped_keys, sealed_keys, created_at)
         VALUES ($1, $2, $3, now())",
    )
    .bind(did)
    .bind(wrapped_keys)
    .bind(sealed_keys)
    .execute(pool)
    .await
    .map(|_| ())
}

/// One row's two blobs, by DID.
async fn blobs_of(pool: &PgPool, did: &str) -> (Option<Vec<u8>>, Option<Vec<u8>>) {
    sqlx::query_as("SELECT wrapped_keys, sealed_keys FROM account_keys WHERE did = $1")
        .bind(did)
        .fetch_one(pool)
        .await
        .expect("read a row's blobs")
}

/// A database migrated through the envelope and nothing after it.
async fn through_the_envelope() -> (PgPool, impl Sized) {
    let (pool, container) = bare_pool().await;
    run_migrations_through(&pool, CUSTODY_ENVELOPE).await;
    (pool, container)
}

#[tokio::test]
async fn every_existing_row_keeps_its_wrapped_keys_and_has_no_sealed_keys() {
    let (pool, _container) = bare_pool().await;
    run_migrations_before(&pool, CUSTODY_ENVELOPE).await;
    let seeded = legacy_rows();
    for (did, wrapped_keys) in &seeded {
        seed_legacy(&pool, did, wrapped_keys).await;
    }

    run_migrations_through(&pool, CUSTODY_ENVELOPE).await;

    for (did, wrapped_keys) in &seeded {
        let (wrapped_after, sealed_after) = blobs_of(&pool, did).await;
        assert_eq!(
            wrapped_after.as_ref(),
            Some(wrapped_keys),
            "{did} keeps its 0.1 ciphertext byte for byte"
        );
        assert_eq!(sealed_after, None, "{did} has no sealed envelope yet");
    }
}

#[tokio::test]
async fn a_row_with_only_sealed_keys_is_accepted() {
    let (pool, _container) = through_the_envelope().await;
    let sealed_keys = b"sealed envelope".to_vec();

    try_insert(
        &pool,
        "did:plc:custodysealed",
        None,
        Some(sealed_keys.as_slice()),
    )
    .await
    .expect("a row with only sealed keys is stored");

    let stored = blobs_of(&pool, "did:plc:custodysealed").await;
    let expected = (None, Some(sealed_keys));
    assert_eq!(stored, expected, "the row reads back as stored");
}

#[tokio::test]
async fn a_row_with_neither_blob_is_refused() {
    let (pool, _container) = through_the_envelope().await;

    let refusal = try_insert(&pool, "did:plc:custodyempty", None, None)
        .await
        .expect_err("a row with neither blob is refused");

    let constraint = refusal
        .as_database_error()
        .and_then(|error| error.constraint());
    assert_eq!(constraint, Some(HOLDS_KEYS), "refused by {HOLDS_KEYS}");
}

#[tokio::test]
async fn the_table_check_reads_false_on_a_bare_database_and_true_once_built() {
    let (pool, _container) = bare_pool().await;
    let bare = key_store::table_exists(&pool)
        .await
        .expect("probe a bare db");
    assert!(!bare, "a bare database has no custody table");

    run_migrations_before(&pool, CUSTODY_ENVELOPE).await;
    let built = key_store::table_exists(&pool)
        .await
        .expect("probe a built db");
    assert!(built, "the custody table exists before the envelope");
}

#[tokio::test]
async fn the_legacy_probe_reads_every_row_by_did_on_the_schema_before_the_envelope() {
    let (pool, _container) = bare_pool().await;
    run_migrations_before(&pool, CUSTODY_ENVELOPE).await;
    let seeded = legacy_rows();
    for (did, wrapped_keys) in &seeded {
        seed_legacy(&pool, did, wrapped_keys).await;
    }

    let probed = key_store::legacy_probe(&pool)
        .await
        .expect("the probe runs before the envelope");

    let read: Vec<(String, Vec<u8>)> = probed
        .into_iter()
        .map(|row| (row.did, row.wrapped_keys))
        .collect();
    let [(zulu, zulu_keys), (alpha, alpha_keys)] = seeded;
    let expected = vec![(alpha.to_owned(), alpha_keys), (zulu.to_owned(), zulu_keys)];
    assert_eq!(read, expected, "every 0.1 row, in DID order");
}

#[tokio::test]
async fn the_legacy_probe_skips_a_row_with_only_sealed_keys() {
    let (pool, _container) = through_the_envelope().await;
    seed_legacy(&pool, "did:plc:custodylegacy", b"legacy").await;
    try_insert(
        &pool,
        "did:plc:custodysealed",
        None,
        Some(b"sealed".as_slice()),
    )
    .await
    .expect("store a sealed-only row");

    let probed = key_store::legacy_probe(&pool).await.expect("probe");

    let dids: Vec<String> = probed.into_iter().map(|row| row.did).collect();
    assert_eq!(dids, ["did:plc:custodylegacy"], "only the 0.1 row");
}

#[tokio::test]
async fn the_unsealed_reads_and_count_skip_a_sealed_row() {
    let (pool, _container) = through_the_envelope().await;
    seed_legacy(&pool, "did:plc:custodyzulu", b"zulu").await;
    seed_legacy(&pool, "did:plc:custodyalpha", b"alpha").await;
    try_insert(
        &pool,
        "did:plc:custodymid",
        Some(b"mid legacy".as_slice()),
        Some(b"mid sealed".as_slice()),
    )
    .await
    .expect("store a row holding both blobs");

    let mut tx = pool.begin().await.expect("begin");
    let unsealed = key_store::legacy_unsealed(&mut *tx)
        .await
        .expect("the locked read");
    let count = key_store::count_unsealed(&mut *tx).await.expect("count");
    tx.rollback().await.expect("rollback");

    let read: Vec<(String, Vec<u8>)> = unsealed
        .into_iter()
        .map(|row| (row.did, row.wrapped_keys))
        .collect();
    let expected = vec![
        ("did:plc:custodyalpha".to_owned(), b"alpha".to_vec()),
        ("did:plc:custodyzulu".to_owned(), b"zulu".to_vec()),
    ];
    assert_eq!(read, expected, "every unsealed row, in DID order");
    assert_eq!(count, 2, "the sealed row is not counted");
}

#[tokio::test]
async fn the_sealed_envelope_is_written_once() {
    let (pool, _container) = through_the_envelope().await;
    seed_legacy(&pool, "did:plc:custodyonce", b"legacy").await;

    let first = key_store::store_sealed(&pool, "did:plc:custodyonce", Some(b"first".as_slice()))
        .await
        .expect("the first write");
    let second = key_store::store_sealed(&pool, "did:plc:custodyonce", Some(b"second".as_slice()))
        .await
        .expect("the second write");
    let missing = key_store::store_sealed(&pool, "did:plc:custodynobody", Some(b"none".as_slice()))
        .await
        .expect("a write to a missing row");

    assert_eq!(first, 1, "the first write changes the row");
    assert_eq!(second, 0, "a sealed row is never written again");
    assert_eq!(missing, 0, "a missing row changes nothing");
    let stored = blobs_of(&pool, "did:plc:custodyonce").await;
    let expected = (Some(b"legacy".to_vec()), Some(b"first".to_vec()));
    assert_eq!(
        stored, expected,
        "the first envelope stays, the 0.1 one too"
    );
}
