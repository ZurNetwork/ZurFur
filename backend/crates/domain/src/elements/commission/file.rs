//! Commission **file entries** (ZMVP-88): intermediate work-in-progress a
//! Participant uploads into the review loop (DESIGN/Commission — "File entries and
//! Markup"). A file entry is **not** a Product — no fact-lock, no atproto: it is
//! private, Index-side, Total-tier content that lives entirely behind the platform
//! (the public-node test never lets it cross to a PDS).
//!
//! Two homes hold a file entry, kept apart on purpose:
//!
//! - The **bytes** live behind the [`FileStore`](crate::ports::FileStore) port,
//!   keyed by an **opaque** [`FileKey`] (a freshly minted UUIDv7 handle) — never a
//!   content-addressed [`BlobId`](crate::elements::blob::BlobId)/CID, which is the
//!   *public* PDS boundary's shape (reusing it here would be a category error).
//!   Content-addressing, real storage, size/format policy, and retention are the
//!   future blob-architecture walkthrough's call; the v1 store is a mock/local
//!   implementation behind the port, and a swap keeps these opaque keys valid.
//! - The **record** that a file entry belongs to a commission is the
//!   [`CommissionFile`] row (the Index-canonical private link), which the retrieval
//!   path reads to authorize a participant and which cascades away with the
//!   commission (it is bookkeeping, never a [`Fact`](super::fact::Fact)). Its
//!   filename/mime/size ride the [`FileStore`] metadata and the `file_added`
//!   changelog entry's payload — the entry renders a sentence without joins (the
//!   Changelog DD's core-renderable rule).

use std::ops::Deref;

use super::CommissionId;
use crate::{datetime::DateTimeUtc, elements::user::UserId};

/// The app-private, **opaque** handle for a stored file entry's bytes — a UUIDv7
/// wrapped for type safety, the key both the [`FileStore`](crate::ports::FileStore)
/// and the [`CommissionFile`] row are keyed by.
///
/// Deliberately **not** a content-address (a
/// [`BlobId`](crate::elements::blob::BlobId)/CID): file entries are private,
/// Index-side content that never touches atproto, and hashing multi-megabyte
/// uploads buys nothing in the v1 mock. A future content-addressed store swaps
/// behind the port with these opaque keys still valid as handles (ZMVP-88, ruling
/// E13). Deref exposes the inner UUID for foreign keys and lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileKey(uuid::Uuid);

impl FileKey {
    /// Wrap an already-minted UUIDv7. Mirrors [`CommissionId::new`]: the app mints
    /// the key (PG16 has no native `uuidv7()`), the domain only names it.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }

    /// Mint a fresh opaque key (`Uuid::now_v7()`) for a new upload — the one place
    /// a file entry's identity is born.
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

/// A file entry's **filename**, validated on the way in — a save-name hint served
/// back in the download's `Content-Disposition` header.
///
/// The gate is security-shaped, not cosmetic: the value crosses into an HTTP
/// header, so a control character (CR/LF) would be header injection, and a path
/// separator has no place in a filename (defense-in-depth against any downstream
/// path use). What is enforced at construction: trimmed, non-empty, at most
/// [`MAX_BYTES`](Self::MAX_BYTES) bytes, free of control characters, and free of
/// `/` or `\`. Non-ASCII is allowed (stored verbatim; the download path emits it
/// RFC 5987-encoded).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileName(String);

/// Why a string was rejected as a [`FileName`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileNameError {
    /// Empty once trimmed. Example: `""` or `"   "`.
    Empty,
    /// Longer than [`FileName::MAX_BYTES`] bytes after trimming.
    TooLong,
    /// Contains a control character (newline, tab, NUL, …) — a header-injection
    /// vector on the download path.
    ControlCharacter,
    /// Contains a path separator (`/` or `\`) — a filename is a name, not a path.
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
    /// The length cap, in bytes — the common filesystem `NAME_MAX`, generous for a
    /// save-name and tight enough to stay a name.
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

/// The caller-supplied metadata carried alongside a file entry's bytes — the
/// [`FileStore`](crate::ports::FileStore)'s `put`/`get` payload (ruling E13).
///
/// `content_type` is **normalized** at construction: a blank or control-bearing
/// MIME becomes `application/octet-stream`, so what is stored is always a safe
/// header value. Combined with the download path's `Content-Disposition:
/// attachment` + `X-Content-Type-Options: nosniff`, a stored SVG/HTML can never
/// execute in the app origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMetadata {
    /// The validated save-name (see [`FileName`]).
    pub filename: FileName,
    /// The normalized MIME type served back as `Content-Type` — never blank, never
    /// control-bearing. See [`FileMetadata::new`].
    pub content_type: String,
    /// The byte length of the stored content, carried for display and for the
    /// changelog entry's payload.
    pub byte_size: i64,
}

impl FileMetadata {
    /// The default MIME when the caller supplies none (or an unusable one): the
    /// safe, opaque `application/octet-stream`.
    pub const DEFAULT_CONTENT_TYPE: &str = "application/octet-stream";

    /// Build metadata, normalizing `content_type`: a value that is blank once
    /// trimmed, or carries a control character, is replaced with
    /// [`DEFAULT_CONTENT_TYPE`](Self::DEFAULT_CONTENT_TYPE) — so the stored MIME is
    /// always a valid, injection-free header value.
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

/// A file entry's bytes plus its metadata, as read back from the
/// [`FileStore`](crate::ports::FileStore) — the `get` result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFile {
    /// The metadata the entry was stored with.
    pub metadata: FileMetadata,
    /// The stored content.
    pub bytes: Vec<u8>,
}

/// The Index-canonical record that a file entry belongs to a commission (ZMVP-88)
/// — the private link the retrieval path reads to authorize a participant and to
/// (later) enumerate a commission's blobs for the hard-delete cascade.
///
/// Deliberately **not a fact** (Deletion DD `3014657`): it is commission-owned
/// bookkeeping that cascades away with the commission (`ON DELETE CASCADE`), so a
/// commission with only file entries stays hard-deletable (AC2 —
/// [`commission_has_facts`](crate::ports::CommissionWrites::commission_has_facts)
/// stays `false`). Its `id` is the same opaque [`FileKey`] the bytes are stored
/// under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionFile {
    /// The file entry's opaque key — the [`FileStore`](crate::ports::FileStore)
    /// handle and this row's primary key.
    pub id: FileKey,
    /// The commission whose review loop this entry joined.
    pub commission_id: CommissionId,
    /// The Participant who uploaded it. Deliberately carries no foreign key onto
    /// users at the database (the changelog precedent): shared history survives a
    /// user tombstone.
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
