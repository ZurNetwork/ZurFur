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

fn keys() -> AccountKeys {
    AccountKeys {
        cold_recovery: SecretKey::new(vec![0xAA; 32]),
        operational: SecretKey::new(vec![0xBB; 32]),
        signing: SecretKey::new(vec![0xCC; 32]),
    }
}

#[tokio::test]
async fn put_then_get_round_trips_the_keys() {
    let (pool, _container) = fresh_pool().await;
    let store = PgKeyStore::new(pool, RootKey::from_bytes(&[9u8; 32]).unwrap());
    let did = Did::new("did:plc:alice".to_string());

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
    let did = Did::new("did:plc:bob".to_string());
    store.put(&did, &keys()).await.unwrap();

    // Read the raw stored bytes and assert none of the three plaintext key runs
    // appear — the column holds ciphertext, never the secp256k1 scalars.
    let wrapped_keys: Vec<u8> =
        sqlx::query_scalar("SELECT wrapped_keys FROM account_keys WHERE did = $1")
            .bind(did.as_str())
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
