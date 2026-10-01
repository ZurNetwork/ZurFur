use super::FileName;

/// Why a string was rejected as a [`FileName`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FileNameError {
    /// Empty once trimmed.
    #[error("filename must not be empty")]
    Empty,
    /// Longer than [`FileName::MAX_BYTES`] bytes after trimming.
    #[error("filename must be at most {} bytes", FileName::MAX_BYTES)]
    TooLong,
    /// Contains a control character — a header-injection vector.
    #[error("filename must not contain control characters")]
    ControlCharacter,
    /// Contains a path separator (`/` or `\`).
    #[error("filename must not contain a path separator")]
    PathSeparator,
}
