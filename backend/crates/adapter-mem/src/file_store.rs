//! In-memory fake of the [`FileStore`] port: a `HashMap<FileKey, StoredBlob>`
//! behind the shared [`MemBackend`].
//!
//! The blob map is shared, never staged — the blob write sits outside the Unit
//! of Work, so a rolled-back unit accepts an orphaned blob. The fake buffers
//! both directions: fidelity to the streaming contract, not to constant memory.

use std::io::Cursor;

use async_trait::async_trait;
use domain::{
    elements::commission::{FileDownload, FileKey, FileMetadata, FileName},
    ports::FileStore,
};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::MemBackend;

/// The mem mirror of a `file_blob` row: bytes alongside their metadata. At-rest
/// data, unlike the port's [`FileDownload`] live reader.
#[derive(Clone)]
pub(crate) struct StoredBlob {
    metadata: FileMetadata,
    bytes: Vec<u8>,
}

/// In-memory [`FileStore`] over the shared [`MemBackend`]'s blob map.
pub struct MemFileStore(pub(crate) MemBackend);

#[async_trait]
impl FileStore for MemFileStore {
    /// Drain `content` and store it under `key` with its finalized
    /// [`FileMetadata`] — an idempotent insert-or-replace, written straight to
    /// the shared map, never staged.
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
