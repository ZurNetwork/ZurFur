use std::ops::Deref;

use serde::Deserialize;

/// The app-private, opaque key of a stored file entry's bytes (UUIDv7) — what
/// both the [`FileStore`](crate::ports::FileStore) and the
/// [`CommissionFile`](super::CommissionFile) row are keyed by. Not a
/// content-address: file entries never touch atproto.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct FileKey(uuid::Uuid);

impl FileKey {
    /// Wrap an already-minted UUIDv7.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// Mint a fresh opaque key for a new upload.
    pub fn generate() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

impl Deref for FileKey {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests;
