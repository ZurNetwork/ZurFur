//! The [`FileStore`] over PostgreSQL (ZMVP-88, streaming seam ZMVP-205): the
//! **v1 local implementation** of the private blob store behind a commission
//! file entry — bytes and caller metadata in a `bytea` table (`file_blob`),
//! keyed by the opaque [`FileKey`]. This is the mock/local store the ticket
//! ships (AC4); the real blob architecture (object storage, limits, formats,
//! retention, content-addressing) is the future blob-architecture walkthrough
//! and swaps behind this same port.
//!
//! **Pool-backed, outside the Unit of Work — by design.** Blob bytes cannot ride
//! a Postgres transaction, and a file entry's atomicity lives elsewhere: the
//! `file_added` changelog entry and the `commission_file` link commit together in
//! the [`UnitOfWork`](domain::ports::UnitOfWork), while this `put` runs **before**
//! that unit as its own step (orphan-on-rollback accepted — nothing points at an
//! orphan). That is why `file_store.rs` is a **documented exception** in the
//! bare-pool-write guard (`tests/no_bare_pool_writes.rs`): its writes have no
//! transactional home, the same reasoning that exempts the profile cache and the
//! key store.
//!
//! **Buffers internally, still — a documented v1 exception.** The port speaks
//! [`tokio::io::AsyncRead`], but this adapter drains it into a `Vec<u8>` before
//! the single `INSERT` (`bytea` gives no other way in) and wraps the read row's
//! `Vec<u8>` in a [`std::io::Cursor`] on the way out. Constant-memory transfer
//! awaits the real blob-architecture swap.
//!
//! The SQL lives in `queries/file/`; the typed functions are generated against
//! the migrated schema (see [`crate::queries`]).

use domain::{
    elements::commission::{FileDownload, FileKey, FileMetadata, FileName},
    ports::FileStore,
};
use sqlx::PgPool;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::queries::file as sql;

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
    /// Drain `content` into a `Vec<u8>`, then upsert it (`INSERT … ON CONFLICT
    /// (key) DO UPDATE`) alongside `filename`/`content_type`/the counted byte
    /// size. An idempotent upsert, so a retried `put` under the same freshly
    /// minted key replaces rather than errors; `created_at` is set only on the
    /// first insert and kept on conflict, so a retry converges on the same
    /// stored row (PR #110 review). Deliberately outside any unit of work
    /// (see the module docs).
    async fn put(
        &self,
        key: FileKey,
        filename: &FileName,
        content_type: &str,
        content: &mut (dyn AsyncRead + Send + Unpin),
    ) -> anyhow::Result<u64> {
        let mut bytes = Vec::new();
        content.read_to_end(&mut bytes).await?;
        let byte_size = bytes.len() as i64;
        sql::put(
            &self.pool,
            *key,
            filename.as_str(),
            content_type,
            byte_size,
            &bytes,
        )
        .await?;
        Ok(bytes.len() as u64)
    }

    /// Read the bytes and metadata stored under `key`, or `None` on a miss, and
    /// wrap the bytes in a [`std::io::Cursor`] as the returned reader. The
    /// stored `filename` is re-validated through [`FileName::try_new`] (the
    /// tamper-surfacing contract the commission read store uses): a value outside
    /// the gate means row tampering and surfaces as an `Err`, never a panic.
    async fn get(&self, key: FileKey) -> anyhow::Result<Option<FileDownload>> {
        let Some(row) = sql::get(&self.pool, *key).await? else {
            return Ok(None);
        };

        Ok(Some(FileDownload {
            metadata: FileMetadata::new(
                FileName::try_new(row.filename)?,
                row.content_type,
                row.byte_size,
            ),
            content: Box::new(std::io::Cursor::new(row.bytes)),
        }))
    }

    /// Remove the bytes under `key`. Idempotent: an absent key matches no row and is
    /// a no-op, the shape the commission hard-delete cascade (ZMVP-66) needs.
    async fn delete(&self, key: FileKey) -> anyhow::Result<()> {
        sql::delete(&self.pool, *key).await?;
        Ok(())
    }
}
