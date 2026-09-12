//! Commission file entries: intermediate work-in-progress a Participant uploads
//! into the review loop. Not a Product — private, Index-side, Total-tier content
//! that never crosses to a PDS.
//!
//! Two homes: the bytes live behind the [`FileStore`](crate::ports::FileStore)
//! port keyed by an opaque [`FileKey`], and the [`CommissionFile`] row records
//! that the entry belongs to a commission.

use std::ops::Deref;

use serde::Deserialize;
use tokio::io::AsyncRead;

use super::CommissionId;
use crate::{datetime::DateTimeUtc, elements::user::UserId};

/// The app-private, opaque key of a stored file entry's bytes (UUIDv7) — what
/// both the [`FileStore`](crate::ports::FileStore) and the [`CommissionFile`]
/// row are keyed by. Not a content-address: file entries never touch atproto.
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

/// A file entry's filename — the save-name hint served back in the download's
/// `Content-Disposition` header. Enforced here: trimmed, non-empty, at most
/// [`MAX_BYTES`](Self::MAX_BYTES) bytes, no control characters (they would be
/// header injection), no `/` or `\`. Non-ASCII is allowed, stored verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileName(String);

/// Why a string was rejected as a [`FileName`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileNameError {
    /// Empty once trimmed.
    Empty,
    /// Longer than [`FileName::MAX_BYTES`] bytes after trimming.
    TooLong,
    /// Contains a control character — a header-injection vector.
    ControlCharacter,
    /// Contains a path separator (`/` or `\`).
    PathSeparator,
}

impl std::fmt::Display for FileNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FileNameError::Empty => write!(f, "filename must not be empty"),
            FileNameError::TooLong => {
                write!(f, "filename must be at most {} bytes", FileName::MAX_BYTES)
            }
            FileNameError::ControlCharacter => {
                write!(f, "filename must not contain control characters")
            }
            FileNameError::PathSeparator => {
                write!(f, "filename must not contain a path separator")
            }
        }
    }
}

impl std::error::Error for FileNameError {}

impl FileName {
    /// The length cap, in bytes — the common filesystem `NAME_MAX`.
    pub const MAX_BYTES: usize = 255;

    /// Validate and wrap a filename: trim, then reject empty, over-cap, any control
    /// character, or a path separator (`/`/`\`).
    ///
    /// ```
    /// use domain::elements::commission::FileName;
    ///
    /// assert_eq!(FileName::try_new("  ref.png  ").unwrap().as_str(), "ref.png");
    /// assert!(FileName::try_new("   ").is_err());        // empty after trim
    /// assert!(FileName::try_new("a\nb.png").is_err());   // control character
    /// assert!(FileName::try_new("../etc/passwd").is_err()); // path separator
    /// ```
    pub fn try_new(raw: impl Into<String>) -> Result<Self, FileNameError> {
        let trimmed = raw.into().trim().to_owned();
        if trimmed.is_empty() {
            return Err(FileNameError::Empty);
        }
        if trimmed.len() > Self::MAX_BYTES {
            return Err(FileNameError::TooLong);
        }
        if trimmed.chars().any(char::is_control) {
            return Err(FileNameError::ControlCharacter);
        }
        if trimmed.contains('/') || trimmed.contains('\\') {
            return Err(FileNameError::PathSeparator);
        }
        Ok(Self(trimmed))
    }

    /// The validated, trimmed filename as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The metadata carried alongside a file entry's bytes — the
/// [`FileStore`](crate::ports::FileStore)'s `put`/`get` payload. `content_type`
/// is normalized at construction, so what is stored is always a safe header
/// value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMetadata {
    /// The validated save-name (see [`FileName`]).
    pub filename: FileName,
    /// The normalized MIME served back as `Content-Type` — never blank, never
    /// control-bearing.
    pub content_type: String,
    /// The byte length of the stored content.
    pub byte_size: i64,
}

impl FileMetadata {
    /// The default MIME when the caller supplies none or an unusable one.
    pub const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";

    /// Build metadata, replacing a blank or control-bearing `content_type` with
    /// [`DEFAULT_CONTENT_TYPE`](Self::DEFAULT_CONTENT_TYPE).
    pub fn new(filename: FileName, content_type: impl Into<String>, byte_size: i64) -> Self {
        let raw = content_type.into();
        let trimmed = raw.trim();
        let content_type = if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
            Self::DEFAULT_CONTENT_TYPE.to_owned()
        } else {
            trimmed.to_owned()
        };
        Self {
            filename,
            content_type,
            byte_size,
        }
    }
}

/// A file entry's metadata plus a live reader over its bytes — the
/// [`FileStore`](crate::ports::FileStore)'s `get` result.
pub struct FileDownload {
    /// The metadata the entry was stored with.
    pub metadata: FileMetadata,
    /// The stored content, streamed rather than buffered whole.
    pub content: Box<dyn AsyncRead + Send + Unpin>,
}

/// The Index-canonical record that a file entry belongs to a commission — the
/// private link the retrieval path reads to authorize a participant. Not a fact:
/// it cascades away with the commission, so a commission with only file entries
/// stays hard-deletable. (DD 3014657)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionFile {
    /// The file entry's opaque key — the [`FileStore`](crate::ports::FileStore)
    /// handle and this row's primary key.
    pub id: FileKey,
    /// The commission whose review loop this entry joined.
    pub commission_id: CommissionId,
    /// The Participant who uploaded it. Carries no foreign key onto users, so
    /// shared history survives a tombstone.
    pub uploaded_by: UserId,
    /// When the entry was uploaded.
    pub created_at: DateTimeUtc,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_trims_and_rejects_unsafe_input() {
        assert_eq!(
            FileName::try_new(" sketch.png ").unwrap().as_str(),
            "sketch.png"
        );
        // A bare unicode name is fine (served RFC 5987-encoded).
        assert!(FileName::try_new("café.png").is_ok());
        assert_eq!(FileName::try_new("   "), Err(FileNameError::Empty));
        assert_eq!(
            FileName::try_new("x".repeat(FileName::MAX_BYTES + 1)),
            Err(FileNameError::TooLong)
        );
        // Exactly at the cap is fine.
        assert!(FileName::try_new("x".repeat(FileName::MAX_BYTES)).is_ok());
        for bad in ["a\nb.png", "a\tb.png", "a\rb.png", "a\0b.png"] {
            assert_eq!(
                FileName::try_new(bad),
                Err(FileNameError::ControlCharacter),
                "control characters are rejected: {bad:?}",
            );
        }
        for bad in ["../secret", "dir/file.png", "dir\\file.png"] {
            assert_eq!(
                FileName::try_new(bad),
                Err(FileNameError::PathSeparator),
                "path separators are rejected: {bad:?}",
            );
        }
    }

    #[test]
    fn content_type_is_normalized_to_a_safe_header_value() {
        let name = FileName::try_new("art.svg").unwrap();
        // A good MIME is kept verbatim (trimmed).
        assert_eq!(
            FileMetadata::new(name.clone(), "  image/svg+xml  ", 10).content_type,
            "image/svg+xml"
        );
        // Blank or control-bearing MIME falls back to octet-stream (no injection).
        assert_eq!(
            FileMetadata::new(name.clone(), "   ", 10).content_type,
            FileMetadata::DEFAULT_CONTENT_TYPE
        );
        assert_eq!(
            FileMetadata::new(name, "text/html\r\nSet-Cookie: x", 10).content_type,
            FileMetadata::DEFAULT_CONTENT_TYPE
        );
    }

    #[test]
    fn a_file_key_is_opaque_and_round_trips() {
        let raw = uuid::Uuid::now_v7();
        assert_eq!(*FileKey::new(raw), raw);
        assert_ne!(*FileKey::generate(), *FileKey::generate());
    }
}
