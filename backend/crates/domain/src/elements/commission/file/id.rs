use serde::Deserialize;

/// The app-private, opaque key of a stored file entry's bytes (UUIDv7) — what
/// both the [`FileStore`](crate::ports::FileStore) and the
/// [`CommissionFile`](super::CommissionFile) row are keyed by. Not a
/// content-address: file entries never touch atproto.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    Deserialize,
    derive_more::From,
    derive_more::Into,
    derive_more::AsRef,
    derive_more::Display,
    derive_more::FromStr,
)]
#[serde(transparent)]
pub struct FileKey(uuid::Uuid);

impl FileKey {
    /// Mint a fresh opaque key for a new upload.
    pub fn generate() -> Self {
        Self(uuid::Uuid::now_v7())
    }
}

#[cfg(test)]
mod tests;
