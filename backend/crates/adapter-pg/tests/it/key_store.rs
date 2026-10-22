//! The custody [`KeyStore`] on PostgreSQL: it round-trips a DID's keys through the
//! envelope in `sealed_keys`, never writes or reads the 0.1 `wrapped_keys`, keeps
//! the plaintext off disk, binds an envelope to its DID, and fails closed on bytes
//! that are not an envelope. Then the write-once rule, read from the crate's SQL.
//! Requires a container runtime socket.

use std::path::{Path, PathBuf};

use adapter_pg::{PgKeyStore, PgPool};
use domain::{
    elements::{custody::CustodyError, did::Did},
    ports::KeyStore,
};
use test_support::custody::{
    LEGACY_ALICE_BLOB_HEX, LEGACY_ALICE_BLOB_STARTING_0X01_HEX, LEGACY_ALICE_DID, hex_bytes,
    legacy_keys, test_custody,
};

/// The envelope's version byte.
const ENVELOPE_VERSION: u8 = 0x01;

/// How every DID-custody key id starts.
const DID_CUSTODY_KEY_ID_PREFIX: &str = "hkdf:did_custody:";

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// The store over `pool`, sealing under the test custodian's DID-custody key.
fn store(pool: &PgPool) -> PgKeyStore {
    PgKeyStore::new(pool.clone(), test_custody().did_custody)
}

/// A DID from text.
fn did(text: &str) -> Did {
    Did::from(text.to_owned())
}

/// One row's stored columns: the 0.1 ciphertext, the envelope and the key version.
async fn stored_row(pool: &PgPool, did: &str) -> (Option<Vec<u8>>, Option<Vec<u8>>, i32) {
    sqlx::query_as("SELECT wrapped_keys, sealed_keys, key_version FROM account_keys WHERE did = $1")
        .bind(did)
        .fetch_one(pool)
        .await
        .expect("read a custody row")
}

/// Store a row with the two blobs given.
async fn insert_row(
    pool: &PgPool,
    did: &str,
    wrapped_keys: Option<&[u8]>,
    sealed_keys: Option<&[u8]>,
) {
    sqlx::query(
        "INSERT INTO account_keys (did, wrapped_keys, sealed_keys, created_at)
         VALUES ($1, $2, $3, now())",
    )
    .bind(did)
    .bind(wrapped_keys)
    .bind(sealed_keys)
    .execute(pool)
    .await
    .expect("store a custody row");
}

#[tokio::test]
async fn put_then_get_round_trips_the_keys() {
    let (pool, _container) = fresh_pool().await;
    let store = store(&pool);
    let alice = did("did:plc:alice");

    let before = store.get(&alice).await.unwrap();
    assert!(before.is_none(), "unknown DID → None");
    store.put(&alice, &legacy_keys()).await.unwrap();

    let read = store.get(&alice).await.unwrap();
    assert_eq!(read, Some(legacy_keys()));
}

#[tokio::test]
async fn keys_are_encrypted_at_rest_not_plaintext() {
    let (pool, _container) = fresh_pool().await;
    let bob = did("did:plc:bob");
    store(&pool).put(&bob, &legacy_keys()).await.unwrap();

    let (_, sealed_keys, _) = stored_row(&pool, bob.as_ref()).await;

    let sealed_keys = sealed_keys.expect("the envelope is stored");
    for byte in [0xAAu8, 0xBB, 0xCC] {
        let run = vec![byte; 32];
        assert!(
            !sealed_keys
                .windows(32)
                .any(|window| window == run.as_slice()),
            "plaintext key bytes ({byte:#x}) found in sealed_keys — not encrypted at rest"
        );
    }
}

#[tokio::test]
async fn put_writes_only_sealed_keys() {
    let (pool, _container) = fresh_pool().await;
    let alice = did("did:plc:alice");
    store(&pool).put(&alice, &legacy_keys()).await.unwrap();

    let (wrapped_keys, sealed_keys, key_version) = stored_row(&pool, alice.as_ref()).await;

    assert_eq!(wrapped_keys, None, "no 0.1 ciphertext is written");
    assert_eq!(key_version, 1, "the key version keeps its default");
    let sealed_keys = sealed_keys.expect("the envelope is stored");
    assert_eq!(sealed_keys[0], ENVELOPE_VERSION, "the envelope's version");
    let key_id_length = usize::from(sealed_keys[1]);
    let key_id = std::str::from_utf8(&sealed_keys[2..2 + key_id_length]).expect("an ASCII key id");
    assert!(
        key_id.starts_with(DID_CUSTODY_KEY_ID_PREFIX),
        "sealed under the DID-custody key"
    );
}

#[tokio::test]
async fn get_never_reads_wrapped_keys() {
    let (pool, _container) = fresh_pool().await;
    let alice_blob = hex_bytes(LEGACY_ALICE_BLOB_HEX);
    insert_row(&pool, LEGACY_ALICE_DID, Some(alice_blob.as_slice()), None).await;

    let read = store(&pool).get(&did(LEGACY_ALICE_DID)).await;

    let error = read.expect_err("a row with no envelope is an error, not keys or None");
    let custody_error = error.downcast_ref::<CustodyError>();
    assert_eq!(custody_error, Some(&CustodyError::NotRewrapped));
}

#[tokio::test]
async fn a_legacy_blob_starting_with_0x01_fails_closed() {
    let (pool, _container) = fresh_pool().await;
    let blob = hex_bytes(LEGACY_ALICE_BLOB_STARTING_0X01_HEX);
    insert_row(&pool, LEGACY_ALICE_DID, None, Some(blob.as_slice())).await;

    let read = store(&pool).get(&did(LEGACY_ALICE_DID)).await;

    assert!(
        read.is_err(),
        "a 0.1 blob in sealed_keys never opens to keys"
    );
}

#[tokio::test]
async fn an_envelope_moved_onto_another_did_does_not_open() {
    let (pool, _container) = fresh_pool().await;
    let store = store(&pool);
    store
        .put(&did("did:plc:alice"), &legacy_keys())
        .await
        .unwrap();
    let (_, alice_envelope, _) = stored_row(&pool, "did:plc:alice").await;
    insert_row(&pool, "did:plc:mallory", None, alice_envelope.as_deref()).await;

    let read = store.get(&did("did:plc:mallory")).await;

    assert!(
        read.is_err(),
        "an envelope opens only for the DID it was sealed for"
    );
}

/// Every `.sql` file under `dir`, recursively.
fn sql_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir).expect("read a SQL directory");
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            files.extend(sql_files(&path));
        } else if path.extension().is_some_and(|extension| extension == "sql") {
            files.push(path);
        }
    }
    files
}

/// The statements of a SQL text, lowercased, without `--` comments, each a list of
/// its words.
fn statements(sql: &str) -> Vec<Vec<String>> {
    let without_comments: String = sql
        .lines()
        .map(|line| line.split_once("--").map_or(line, |(code, _)| code))
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase();
    without_comments
        .split(';')
        .map(|statement| {
            statement
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
                .filter(|word| !word.is_empty())
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .filter(|words| !words.is_empty())
        .collect()
}

/// Whether a statement assigns `sealed_keys` in a `SET` clause: an `UPDATE`, or an
/// upsert's `DO UPDATE`.
fn assigns_sealed_keys(words: &[String]) -> bool {
    let Some(set) = words.iter().position(|word| word == "set") else {
        return false;
    };
    let set_clause_end = words[set..]
        .iter()
        .position(|word| word == "where")
        .map_or(words.len(), |offset| set + offset);
    words[set..set_clause_end]
        .iter()
        .any(|word| word == "sealed_keys")
}

/// Whether a statement only changes a row whose `sealed_keys` is still NULL.
fn carries_the_null_guard(words: &[String]) -> bool {
    let guard = ["and", "sealed_keys", "is", "null"];
    words.windows(guard.len()).any(|window| window == guard)
}

#[test]
fn sealed_keys_has_one_guarded_update() {
    let guarded = statements(
        "UPDATE account_keys SET sealed_keys = $2 WHERE did = $1 AND sealed_keys IS NULL",
    );
    let unguarded = statements("UPDATE account_keys SET sealed_keys = $2 WHERE did = $1");
    let insert =
        statements("INSERT INTO account_keys (did, sealed_keys, created_at) VALUES ($1, $2, $3)");
    let filter = statements("SELECT did FROM t WHERE sealed_keys IS NULL");
    assert!(assigns_sealed_keys(&guarded[0]) && carries_the_null_guard(&guarded[0]));
    assert!(assigns_sealed_keys(&unguarded[0]) && !carries_the_null_guard(&unguarded[0]));
    assert!(
        !assigns_sealed_keys(&insert[0]),
        "the INSERT is not an update"
    );
    assert!(!assigns_sealed_keys(&filter[0]), "a filter is not a write");

    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut files = sql_files(&crate_dir.join("queries"));
    files.extend(sql_files(&crate_dir.join("migrations")));
    assert!(files.len() > 100, "the scan found the crate's SQL files");
    let mut writers = Vec::new();
    for file in &files {
        let sql = std::fs::read_to_string(file).expect("read a SQL file");
        for words in statements(&sql) {
            if assigns_sealed_keys(&words) {
                writers.push((file.display().to_string(), carries_the_null_guard(&words)));
            }
        }
    }

    assert_eq!(
        writers.len(),
        1,
        "one statement updates sealed_keys: {writers:#?}"
    );
    let (writer, guarded) = &writers[0];
    assert!(
        writer.ends_with("queries/key_store/store_sealed.sql"),
        "the one writer is {writer}"
    );
    assert!(
        guarded,
        "the one update writes only while sealed_keys IS NULL"
    );
}
