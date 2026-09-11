//! In-memory fake of the [`FileStore`] port (ZMVP-88, streaming seam
//! ZMVP-205): the file-entry **blob** store, a `HashMap<FileKey, StoredBlob>`
//! behind the shared [`MemBackend`]. The test/dev twin of `adapter-pg`'s
//! `PgFileStore`.
//!
//! **Shared, not staged.** Like the profile cache, the blob map's `Arc` is
//! cloned (not deep-copied) into a unit of work's staging snapshot, because
//! the blob write is a step *outside* the Unit of Work — bytes cannot ride a
//! transaction, and a unit that rolls back accepts leaving the blob orphaned
//! (nothing points at it).
//!
//! **Buffers by definition.** The fake drains a `put`'s reader into a
//! `Vec<u8>` and wraps a `get`'s stored `Vec<u8>` in a [`Cursor`] — fidelity
//! to the port's streaming *contract*, not to constant-memory transfer (v1's
//! pg adapter buffers internally too; see its module docs).

use std::io::Cursor;

use async_trait::async_trait;
use domain::{
    elements::commission::{FileDownload, FileKey, FileMetadata, FileName},
    ports::FileStore,
};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::MemBackend;

/// The mem mirror of a `file_blob` row: bytes alongside the metadata they
/// were stored with. Kept apart from the port's [`FileDownload`] — that is a
/// live reader, this is at-rest data.
#[derive(Clone)]
pub(crate) struct StoredBlob {
    metadata: FileMetadata,
    bytes: Vec<u8>,
}

/// In-memory [`FileStore`] over the shared [`MemBackend`]'s blob map (ZMVP-88).
pub struct MemFileStore(pub(crate) MemBackend);

#[async_trait]
impl FileStore for MemFileStore {
    /// Drain `content` into a `Vec<u8>` and store it under `key` alongside
    /// its finalized [`FileMetadata`] — an idempotent insert-or-replace,
    /// straight through to the shared map (never staged): the blob write is
    /// a Unit-of-Work exemption.
    async fn put(
        &self,
        key: FileKey,
        filename: &FileName,
        content_type: &str,
        content: &mut (dyn AsyncRead + Send + Unpin),
    ) -> anyhow::Result<u64> {
        let mut bytes = Vec::new();
        content.read_to_end(&mut bytes).await?;
        let len = bytes.len();
        let metadata = FileMetadata::new(filename.clone(), content_type, len as i64);
        let mut blobs = self
            .0
            .blobs
            .lock()
            .expect("MemBackend blobs mutex poisoned");
        blobs.insert(key, StoredBlob { metadata, bytes });
        Ok(len as u64)
    }

    /// Wrap the stored bytes in a [`Cursor`] as the returned reader, or
    /// `None` on a miss.
    async fn get(&self, key: FileKey) -> anyhow::Result<Option<FileDownload>> {
        let blobs = self
            .0
            .blobs
            .lock()
            .expect("MemBackend blobs mutex poisoned");
        Ok(blobs.get(&key).map(|stored| FileDownload {
            metadata: stored.metadata.clone(),
            content: Box::new(Cursor::new(stored.bytes.clone())),
        }))
    }

    /// Remove the bytes under `key`. Idempotent: an absent key is a no-op.
    async fn delete(&self, key: FileKey) -> anyhow::Result<()> {
        let mut blobs = self
            .0
            .blobs
            .lock()
            .expect("MemBackend blobs mutex poisoned");
        blobs.remove(&key);
        Ok(())
    }
}
