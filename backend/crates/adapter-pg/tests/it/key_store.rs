//! Round-trips the custody [`KeyStore`] against a throwaway PostgreSQL container,
//! proving the migration-created `account_keys` table (1) round-trips a wrapped key
//! bundle, and (2) stores it **encrypted** — the plaintext key bytes never appear
//! in the `wrapped_keys` column. Requires a container runtime socket.

use adapter_pg::{PgKeyStore, PgPool, RootKey};
use domain::{
    elements::{
        account_keys::{AccountKeys, SecretKey},
        did::Did,
    },
    ports::KeyStore,
};

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

fn keys() -> AccountKeys {
    AccountKeys {
        cold_recovery: SecretKey::new(vec![0xAA; 32]),
        operational: SecretKey::new(vec![0xBB; 32]),
        signing: SecretKey::new(vec![0xCC; 32]),
    }
}

/// The root key every 0.1 legacy fixture below is sealed under.
const LEGACY_ROOT: [u8; 32] = [9u8; 32];

/// A 0.1 blob: root `[9; 32]`, `did:plc:alice`, keys 0xAA/0xBB/0xCC.
const LEGACY_ALICE_BLOB_HEX: &str = "8d64dbe72a9240dd603161404dde03a52f2ab5f09d01c2a73c407435224dd5a5e90b4a81e254fbfd2e90bb7ce0b5e9c538a7264805af7e4dfae728cdda82b3a41d5bae953729453b5c051a9492e899424528f87209964566cdceb06afdb612fef8044e0bf9799fd4286f9992dba79223546f5658cec25e6d3139731b2c932f53bef06a6ebb0d403f";

/// A 0.1 blob: root `[9; 32]`, `did:plc:bob`, keys 0xAA/0xBB/0xCC.
const LEGACY_BOB_BLOB_HEX: &str = "ac5871adc37fc19049ccf5d3073dfe6fededfd9b34c141e9757645fded3eed3fa9db90a84dd5f243bef7871349ac7e0a322850734beeed04f3b7081706ca0fb0894be0114d46ccecca099f807e1e8f04c2eb56b1693f68b92060e84d01334ba3fc385c25915807e36c79765293b9bb05d0085015e369625bc9b00c18ffffef71521c6b9c64ec95d3";

/// A 0.1 blob: root `[9; 32]`, `did:plc:alice`, keys 0xAA/0xBB/0xCC; its first byte is 0x01.
const LEGACY_ALICE_BLOB_STARTING_0X01_HEX: &str = "017c34555bb478e165076720364596499e1ab8c224c4a65ec7907f78b40ec4dc48043e5ed67aa6fad1053c4eba0306264a360d7492f45c95252c44fb91401ee504803dc33c351d65a2948edaec168ac913e8aacd09cfdc1ec3e86fcf28ecd37771cd31fe3f52054a6daa4dc49049dc6fdcaf0e61391320a9d4fab3551254e1ae2068ecdc45ba2def";

/// Decodes a lowercase hex fixture into bytes; panics on malformed hex.
fn hex_bytes(hex: &str) -> Vec<u8> {
    assert!(hex.len().is_multiple_of(2), "hex fixture has an odd length");
    hex.as_bytes()
        .chunks(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).expect("hex fixture is ASCII");
            u8::from_str_radix(digits, 16).expect("hex fixture holds only hex digits")
        })
        .collect()
}

#[tokio::test]
async fn put_then_get_round_trips_the_keys() {
    let (pool, _container) = fresh_pool().await;
    let store = PgKeyStore::new(pool, RootKey::from_bytes(&[9u8; 32]).unwrap());
    let did = Did::from("did:plc:alice".to_string());

    assert!(
        store.get(&did).await.unwrap().is_none(),
        "unknown DID → None"
    );
    store.put(&did, &keys()).await.unwrap();
    assert_eq!(store.get(&did).await.unwrap().unwrap(), keys());
}

#[tokio::test]
async fn keys_are_encrypted_at_rest_not_plaintext() {
    let (pool, _container) = fresh_pool().await;
    let store = PgKeyStore::new(pool.clone(), RootKey::from_bytes(&[9u8; 32]).unwrap());
    let did = Did::from("did:plc:bob".to_string());
    store.put(&did, &keys()).await.unwrap();

    // Read the raw stored bytes and assert none of the three plaintext key runs
    // appear — the column holds ciphertext, never the secp256k1 scalars.
    let wrapped_keys: Vec<u8> =
        sqlx::query_scalar("SELECT wrapped_keys FROM account_keys WHERE did = $1")
            .bind(did.as_ref())
            .fetch_one(&pool)
            .await
            .unwrap();
    for byte in [0xAAu8, 0xBB, 0xCC] {
        let run = vec![byte; 32];
        assert!(
            !wrapped_keys.windows(32).any(|w| w == run.as_slice()),
            "plaintext key bytes ({byte:#x}) found in wrapped_keys — not encrypted at rest"
        );
    }
}

#[test]
fn legacy_fixtures_open_under_the_0_1_root() {
    let root = RootKey::from_bytes(&LEGACY_ROOT).unwrap();
    let alice_blob = hex_bytes(LEGACY_ALICE_BLOB_HEX);
    let bob_blob = hex_bytes(LEGACY_BOB_BLOB_HEX);
    let alice_blob_starting_0x01 = hex_bytes(LEGACY_ALICE_BLOB_STARTING_0X01_HEX);
    let expected_keys = keys();

    let alice_keys = root.unwrap("did:plc:alice", &alice_blob).unwrap();
    assert_eq!(alice_keys, expected_keys);
    let bob_keys = root.unwrap("did:plc:bob", &bob_blob).unwrap();
    assert_eq!(bob_keys, expected_keys);
    let alice_keys_from_0x01 = root
        .unwrap("did:plc:alice", &alice_blob_starting_0x01)
        .unwrap();
    assert_eq!(alice_keys_from_0x01, expected_keys);

    let expected_first_byte = 0x01u8;
    assert_eq!(alice_blob_starting_0x01[0], expected_first_byte);

    let alice_blob_as_bob = root.unwrap("did:plc:bob", &alice_blob);
    assert!(
        alice_blob_as_bob.is_err(),
        "alice's blob opened under bob's DID — the DID is not bound"
    );
}
