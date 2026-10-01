use super::FileNameError;

/// A file entry's filename — the save-name hint served back in the download's
/// `Content-Disposition` header. Enforced here: trimmed, non-empty, at most
/// [`MAX_BYTES`](Self::MAX_BYTES) bytes, no control characters (they would be
/// header injection), no `/` or `\`. Non-ASCII is allowed, stored verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileName(String);

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

#[cfg(test)]
mod tests;
