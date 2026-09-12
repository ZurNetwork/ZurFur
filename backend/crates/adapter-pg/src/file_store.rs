//! The [`FileStore`] over PostgreSQL: bytes and metadata in a `bytea` table
//! (`file_blob`), keyed by the opaque [`FileKey`]. Pool-backed, outside the
//! Unit of Work by design — blob bytes can't ride a transaction (a documented
//! exemption in `no_bare_pool_writes.rs`). Buffers internally.

use domain::{
    elements::commission::{FileDownload, FileKey, FileMetadata, FileName},
    ports::FileStore,
};
use sqlx::PgPool;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::queries::file as sql;

/// PostgreSQL [`FileStore`] — the v1 local blob store. Holds the pool directly;
/// its writes are a step outside the domain Unit of Work (see module docs).
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
    /// Drains `content` into a `Vec<u8>`, then upserts it alongside
    /// `filename`/`content_type`/byte size. Idempotent: a retried `put` under
    /// the same key replaces rather than errors, and `created_at` is kept on
    /// conflict. Deliberately outside any unit of work (see module docs).
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

    /// The bytes and metadata stored under `key`, or `None` on a miss; bytes
    /// come back wrapped in a [`std::io::Cursor`]. Re-validates the stored
    /// `filename`; an `Err` on tampering, never a panic.
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

    /// Removes the bytes under `key`. Idempotent: an absent key is a no-op.
    async fn delete(&self, key: FileKey) -> anyhow::Result<()> {
        sql::delete(&self.pool, *key).await?;
        Ok(())
    }
}
