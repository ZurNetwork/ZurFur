use test_support::custody::{
    LEGACY_ALICE_BLOB_HEX, LEGACY_ALICE_DID, LEGACY_BOB_BLOB_HEX, LEGACY_BOB_DID, LEGACY_ROOT,
    hex_bytes, legacy_keys, lying_did_custody, test_custody,
};

use super::*;

/// The 0.1 root the fixtures are sealed under.
fn legacy_root() -> LegacyRootKey {
    LegacyRootKey::new(Zeroizing::new(LEGACY_ROOT))
}

/// A fresh, fully migrated database; the second element keeps it alive.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// Store one 0.1 row: its ciphertext and no envelope.
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

/// Alice's and Bob's 0.1 rows, each sealed for its own DID.
async fn seed_alice_and_bob(pool: &PgPool) {
    seed_legacy(pool, LEGACY_ALICE_DID, &hex_bytes(LEGACY_ALICE_BLOB_HEX)).await;
    seed_legacy(pool, LEGACY_BOB_DID, &hex_bytes(LEGACY_BOB_BLOB_HEX)).await;
}

/// Every row's envelope, by DID.
async fn envelopes(pool: &PgPool) -> Vec<(String, Option<Vec<u8>>)> {
    sqlx::query_as("SELECT did, sealed_keys FROM account_keys ORDER BY did")
        .fetch_all(pool)
        .await
        .expect("read the envelopes")
}

#[test]
fn a_bundle_keeps_the_keys_in_role_order() {
    let keys = legacy_keys();

    let bundle = KeyBundle::try_from(&keys).unwrap();

    let expected_bytes = [[0xAAu8; 32], [0xBB; 32], [0xCC; 32]].concat();
    assert_eq!(bundle.expose(), expected_bytes.as_slice());
    assert_eq!(AccountKeys::from(&bundle), keys);
}

#[test]
fn keys_of_another_length_are_not_a_bundle() {
    let short_signing_key = AccountKeys {
        signing: SecretKey::new(vec![0xCC; 31]),
        ..legacy_keys()
    };

    let from_keys = KeyBundle::try_from(&short_signing_key);
    let from_bytes = KeyBundle::try_from([0u8; 95].as_slice());

    assert_eq!(from_keys.err(), Some(NotAKeyBundle));
    assert_eq!(from_bytes.err(), Some(NotAKeyBundle));
}

#[tokio::test]
async fn one_bad_legacy_row_aborts_the_whole_rewrap() {
    let (pool, _db) = fresh_pool().await;
    seed_legacy(&pool, LEGACY_ALICE_DID, &hex_bytes(LEGACY_ALICE_BLOB_HEX)).await;
    // Alice's ciphertext under Bob's DID: it doesn't open, and Bob sorts after Alice.
    seed_legacy(&pool, LEGACY_BOB_DID, &hex_bytes(LEGACY_ALICE_BLOB_HEX)).await;
    let sealer = test_custody().did_custody;

    let rewrap = PgKeyStore::rewrap_legacy(&pool, &legacy_root(), &sealer).await;

    let refused = matches!(
        rewrap,
        Err(RewrapError::Reseal(ResealError::LegacyUnopenable))
    );
    assert!(refused, "the row that doesn't open stops the re-wrap");
    let expected = vec![
        (LEGACY_ALICE_DID.to_owned(), None),
        (LEGACY_BOB_DID.to_owned(), None),
    ];
    assert_eq!(envelopes(&pool).await, expected, "no row is sealed");
}

#[tokio::test]
async fn a_new_envelope_that_opens_differently_aborts_the_rewrap() {
    let (pool, _db) = fresh_pool().await;
    seed_alice_and_bob(&pool).await;
    let lying_sealer = lying_did_custody();

    let rewrap = PgKeyStore::rewrap_legacy(&pool, &legacy_root(), &lying_sealer).await;

    let refused = matches!(
        rewrap,
        Err(RewrapError::Reseal(ResealError::SelfCheckMismatch))
    );
    assert!(
        refused,
        "an envelope that opens to other keys stops the re-wrap"
    );
    let expected = vec![
        (LEGACY_ALICE_DID.to_owned(), None),
        (LEGACY_BOB_DID.to_owned(), None),
    ];
    assert_eq!(envelopes(&pool).await, expected, "no row is sealed");
}

#[tokio::test]
async fn two_concurrent_rewraps_seal_each_row_once() {
    let (pool, _db) = fresh_pool().await;
    seed_alice_and_bob(&pool).await;
    let sealer = test_custody().did_custody;
    let root = legacy_root();

    let (first, second) = tokio::join!(
        PgKeyStore::rewrap_legacy(&pool, &root, &sealer),
        PgKeyStore::rewrap_legacy(&pool, &root, &sealer),
    );

    let first = first.expect("the first re-wrap commits");
    let second = second.expect("the second re-wrap commits");
    assert_eq!(first + second, 2, "each row is sealed by one re-wrap only");
    let store = PgKeyStore::new(pool.clone(), sealer);
    for did in [LEGACY_ALICE_DID, LEGACY_BOB_DID] {
        let keys = store.get(&Did::from(did.to_owned())).await.unwrap();
        assert_eq!(
            keys,
            Some(legacy_keys()),
            "{did}'s envelope opens to its keys"
        );
    }
}
