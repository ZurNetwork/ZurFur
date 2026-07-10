//! In-memory fake of the [`FileStore`] port (ZMVP-88): the file-entry **blob**
//! store, a `HashMap<FileKey, StoredFile>` behind the shared [`MemBackend`]. The
//! test/dev twin of `adapter-pg`'s `PgFileStore`, and the established "fake home"
//! the ruling names.
//!
//! **Shared, not staged.** Like the profile cache, the blob map's `Arc` is cloned
//! (not deep-copied) into a unit of work's staging snapshot, because the blob write
//! is a step *outside* the Unit of Work — bytes cannot ride a transaction, and a
//! unit that rolls back accepts leaving the blob orphaned (nothing points at it).

use async_trait::async_trait;
use domain::{
    elements::commission::{FileKey, FileMetadata, StoredFile},
    ports::FileStore,
};

use crate::MemBackend;

/// In-memory [`FileStore`] over the shared [`MemBackend`]'s blob map (ZMVP-88).
pub struct MemFileStore(pub(crate) MemBackend);

#[async_trait]
impl FileStore for MemFileStore {
    /// Store the bytes and metadata under `key` — an idempotent insert-or-replace,
    /// the mem mirror of the pg upsert. Writes straight through to the shared map
    /// (never a staged snapshot): the blob write is a Unit-of-Work exemption.
    async fn put(&self, key: FileKey, metadata: &FileMetadata, bytes: &[u8]) -> anyhow::Result<()> {
        let mut blobs = self
            .0
            .blobs
            .lock()
            .expect("MemBackend blobs mutex poisoned");
        blobs.insert(
            key,
            StoredFile {
                metadata: metadata.clone(),
                bytes: bytes.to_vec(),
            },
        );
        Ok(())
    }

    /// Read the bytes and metadata under `key`, or `None` on a miss.
    async fn get(&self, key: FileKey) -> anyhow::Result<Option<StoredFile>> {
        let blobs = self
            .0
            .blobs
            .lock()
            .expect("MemBackend blobs mutex poisoned");
        Ok(blobs.get(&key).cloned())
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
