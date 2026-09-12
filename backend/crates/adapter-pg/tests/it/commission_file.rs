//! Commission file entries over PostgreSQL, against a throwaway
//! container: the `commission_file` link (written on the Unit of Work, read scoped
//! to its commission) and the `FileStore` blob store (`PgFileStore`, pool-backed).
//! Also pins AC2 at the store layer — a commission with only file entries still
//! answers `commission_has_facts == false` (the schema tripwire in
//! `commission.rs` proves `commission_file` is classified NON-FACT). Requires a
//! container runtime socket (DOCKER_HOST honored).

use std::io::Cursor;

use adapter_pg::{PgDatabase, PgFileStore, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            Commission, CommissionFile, CommissionId, CommissionTitle, FileKey, FileName,
        },
        did::Did,
        user::User,
    },
    ports::{Database, FileStore},
};

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

async fn provision(pool: &PgPool, did: &str) -> User {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// Create and commit a commission owned by `owner`, returning its id.
async fn seed_commission(pool: &PgPool, owner: &User, title: &str) -> CommissionId {
    let db = PgDatabase::new(pool.clone());
    let commission = Commission::create(
        title.parse::<CommissionTitle>().expect("title"),
        owner.id.clone(),
        Utc::now(),
        None,
    );
    let id = commission.id;
    let mut uow = db.begin().await.expect("begin");
    uow.commissions().create(&commission).await.expect("create");
    uow.commit().await.expect("commit");
    id
}

// The link round-trips (written on the unit of work, read scoped to its
// commission), a commission with only file entries still bears NO facts (AC2), and
// a key belonging to another commission is invisible (no cross-commission oracle).
#[tokio::test]
async fn file_link_round_trips_scoped_and_is_not_a_fact() {
    let (pool, _c) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:file-owner").await;
    let mine = seed_commission(&pool, &owner, "Mine").await;
    let other = seed_commission(&pool, &owner, "Other").await;

    let key = FileKey::generate();
    let file = CommissionFile {
        id: key,
        commission_id: mine,
        uploaded_by: owner.id.clone(),
        created_at: Utc::now(),
    };

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        commissions.add_file(&file).await.expect("add_file");
        // AC2 — in the SAME unit that added the file entry, the commission still
        // bears no facts (a file entry is bookkeeping, not a Product).
        assert!(
            !commissions
                .commission_has_facts(&mine)
                .await
                .expect("has_facts"),
            "a file entry must not trip fact-lock",
        );
    }
    uow.commit().await.expect("commit");

    let store = adapter_pg::PgCommissionStore::new(pool.clone());
    use domain::ports::CommissionStore;
    let found = store
        .find_file(&mine, key)
        .await
        .expect("find_file")
        .expect("present");
    assert_eq!(found.id, key);
    assert_eq!(found.commission_id, mine);
    assert_eq!(found.uploaded_by, owner.id);

    // Scoped: the same key asked under a different commission is None.
    assert!(
        store
            .find_file(&other, key)
            .await
            .expect("find_file other")
            .is_none(),
        "a file key is invisible outside its own commission",
    );
    // An unknown key is None.
    assert!(
        store
            .find_file(&mine, FileKey::generate())
            .await
            .expect("find_file unknown")
            .is_none(),
    );

    // And after commit, from a later unit, facts still false.
    let mut uow = db.begin().await.expect("begin later");
    assert!(
        !uow.commissions()
            .commission_has_facts(&mine)
            .await
            .expect("has_facts later"),
    );
    uow.rollback().await.expect("rollback read-only");
}

// The FileStore drains and counts a streamed `put`, streams the bytes back
// out on `get`, and delete removes them idempotently. Pool-backed — the blob
// write is a step outside the unit of work.
#[tokio::test]
async fn file_store_put_get_delete_round_trip() {
    let (pool, _c) = fresh_pool().await;
    let store = PgFileStore::new(pool.clone());

    let key = FileKey::generate();
    let filename = FileName::try_new("art.svg").unwrap();
    let bytes = b"<svg>".to_vec();

    assert!(
        store.get(key).await.expect("get miss").is_none(),
        "absent before put"
    );

    let written = store
        .put(
            key,
            &filename,
            "image/svg+xml",
            &mut Cursor::new(bytes.clone()),
        )
        .await
        .expect("put");
    assert_eq!(written, bytes.len() as u64, "put counts the bytes itself");

    let got = store.get(key).await.expect("get").expect("present");
    assert_eq!(got.metadata.filename.as_str(), "art.svg");
    assert_eq!(got.metadata.content_type, "image/svg+xml");
    assert_eq!(got.metadata.byte_size, bytes.len() as i64);
    assert_eq!(read_all(got.content).await, bytes);

    // put is an idempotent upsert.
    let bytes2 = b"<svg/>".to_vec();
    store
        .put(
            key,
            &filename,
            "image/svg+xml",
            &mut Cursor::new(bytes2.clone()),
        )
        .await
        .expect("re-put");
    let got2 = store.get(key).await.unwrap().unwrap();
    assert_eq!(read_all(got2.content).await, bytes2);

    store.delete(key).await.expect("delete");
    assert!(store.get(key).await.expect("get after delete").is_none());
    // Deleting an absent key is a no-op, not an error.
    store.delete(key).await.expect("idempotent delete");
}

/// Drains a downloaded reader into a `Vec<u8>` for a byte-equality assertion.
async fn read_all(mut content: Box<dyn tokio::io::AsyncRead + Send + Unpin>) -> Vec<u8> {
    use tokio::io::AsyncReadExt;
    let mut buf = Vec::new();
    content
        .read_to_end(&mut buf)
        .await
        .expect("read the downloaded content");
    buf
}
