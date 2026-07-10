//! The [`FileStore`] over PostgreSQL (ZMVP-88): the **v1 local implementation** of
//! the private blob store behind a commission file entry — bytes and caller
//! metadata in a `bytea` table (`file_blob`), keyed by the opaque [`FileKey`]. This
//! is the mock/local store the ticket ships (AC4); the real blob architecture
//! (object storage, limits, formats, retention, content-addressing) is the future
//! blob-architecture walkthrough and swaps behind this same port.
//!
//! **Pool-backed, outside the Unit of Work — by design.** Blob bytes cannot ride a
//! Postgres transaction, and a file entry's atomicity lives elsewhere: the
//! `file_added` changelog entry and the `commission_file` link commit together in
//! the [`UnitOfWork`](domain::ports::UnitOfWork), while this `put` runs **before**
//! that unit as its own step (orphan-on-rollback accepted — nothing points at an
//! orphan). That is why `file_store.rs` is a **documented exception** in the
//! bare-pool-write guard (`tests/no_bare_pool_writes.rs`): its writes have no
//! transactional home, the same reasoning that exempts the profile cache and the
//! key store.

use domain::{
    elements::commission::{FileKey, FileMetadata, FileName, StoredFile},
    ports::FileStore,
};
use sqlx::{PgPool, query};

/// PostgreSQL [`FileStore`] — the v1 local blob store. Holds the pool directly
/// (`&self`, like the profile cache): its writes are a step outside the domain Unit
/// of Work (see the module docs), never a transaction-bound view.
pub struct PgFileStore {
    pool: PgPool,
}

impl PgFileStore {
    /// Wraps a [`PgPool`] as a [`FileStore`]. Clones the pool handle (cheap — it's
    /// an `Arc`), so the caller keeps its own.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl FileStore for PgFileStore {
    /// Store the bytes and metadata under `key` (`INSERT … ON CONFLICT (key) DO
    /// UPDATE`) — an idempotent upsert, so a retried `put` under the same freshly
    /// minted key replaces rather than errors. `created_at` is set only on the
    /// first insert and kept on conflict, so a retry converges on the same stored
    /// row (PR #110 review). A single-statement write on the pool, deliberately
    /// outside any unit of work (see the module docs).
    async fn put(&self, key: FileKey, metadata: &FileMetadata, bytes: &[u8]) -> anyhow::Result<()> {
        query!(
            r#"
            INSERT INTO file_blob (key, filename, content_type, byte_size, bytes, created_at)
            VALUES ($1, $2, $3, $4, $5, now())
            ON CONFLICT (key) DO UPDATE
              SET filename = EXCLUDED.filename,
                  content_type = EXCLUDED.content_type,
                  byte_size = EXCLUDED.byte_size,
                  bytes = EXCLUDED.bytes
            "#,
            *key,
            metadata.filename.as_str(),
            metadata.content_type,
            metadata.byte_size,
            bytes,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Read the bytes and metadata stored under `key`, or `None` on a miss. The
    /// stored `filename` is re-validated through [`FileName::try_new`] (the
    /// tamper-surfacing contract the commission read store uses): a value outside
    /// the gate means row tampering and surfaces as an `Err`, never a panic.
    async fn get(&self, key: FileKey) -> anyhow::Result<Option<StoredFile>> {
        let Some(row) = query!(
            r#"
            SELECT filename, content_type, byte_size, bytes
            FROM file_blob
            WHERE key = $1
            "#,
            *key,
        )
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };

        Ok(Some(StoredFile {
            metadata: FileMetadata::new(
                FileName::try_new(row.filename)?,
                row.content_type,
                row.byte_size,
            ),
            bytes: row.bytes,
        }))
    }

    /// Remove the bytes under `key`. Idempotent: an absent key matches no row and is
    /// a no-op, the shape the commission hard-delete cascade (ZMVP-66) needs.
    async fn delete(&self, key: FileKey) -> anyhow::Result<()> {
        query!(r#"DELETE FROM file_blob WHERE key = $1"#, *key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
