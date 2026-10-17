use tokio::io::AsyncRead;

use super::FileKey;
use crate::{datetime::DateTimeUtc, elements::text::StoredText};

/// One file entry as a commission's file list reads it: its key and upload
/// time. Who uploaded it is not part of this read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileSummary {
    /// The file entry's opaque key.
    pub id: FileKey,
    /// When the entry was uploaded.
    pub created_at: DateTimeUtc,
}

/// The metadata carried alongside a file entry's bytes — the
/// [`FileStore`](crate::ports::FileStore)'s `put`/`get` payload. `content_type`
/// is normalized at construction, so what is stored is always a safe header
/// value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMetadata {
    /// The save-name as stored; it passed the [`FileName`](super::FileName) rule
    /// in force at upload, and a load never re-checks it.
    pub filename: StoredText,
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
    pub fn new(filename: StoredText, content_type: impl Into<String>, byte_size: i64) -> Self {
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

#[cfg(test)]
mod tests;
