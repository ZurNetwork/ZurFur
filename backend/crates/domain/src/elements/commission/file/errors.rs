use super::FileName;

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
