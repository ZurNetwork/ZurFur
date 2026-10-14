use super::FileNameError;
use crate::elements::text::StoredText;

/// A file entry's filename — the save-name hint served back in the download's
/// `Content-Disposition` header. Enforced here: trimmed, non-empty, at most
/// [`MAX_BYTES`](Self::MAX_BYTES) bytes, no control characters (they would be
/// header injection), no `/` or `\`, no leading `.` (a dotfile is configuration).
/// Non-ASCII is allowed, stored verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileName(String);

impl FileName {
    /// The length cap, in bytes — the common filesystem `NAME_MAX`.
    pub const MAX_BYTES: usize = 255;

    /// The validated, trimmed filename as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for FileName {
    type Error = FileNameError;

    /// Validate and wrap a filename: trim, then reject empty, over-cap, any control
    /// character, a path separator (`/`/`\`), or a leading `.`.
    ///
    /// ```
    /// use domain::elements::commission::FileName;
    ///
    /// assert_eq!(FileName::try_from("  ref.png  ").unwrap().as_str(), "ref.png");
    /// assert!(FileName::try_from("   ").is_err());        // empty after trim
    /// assert!(FileName::try_from("a\nb.png").is_err());   // control character
    /// assert!(FileName::try_from("../etc/passwd").is_err()); // path separator
    /// assert!(FileName::try_from(".theme.yaml").is_err());  // leading dot
    /// assert!(FileName::try_from("a.b.png").is_ok());       // an interior dot is fine
    /// ```
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        let trimmed = raw.trim().to_owned();
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
        if trimmed.starts_with('.') {
            return Err(FileNameError::LeadingDot);
        }
        Ok(Self(trimmed))
    }
}

impl TryFrom<&str> for FileName {
    type Error = FileNameError;

    fn try_from(raw: &str) -> Result<Self, Self::Error> {
        Self::try_from(raw.to_owned())
    }
}

impl std::str::FromStr for FileName {
    type Err = FileNameError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw)
    }
}

impl From<FileName> for StoredText {
    /// The checked filename, as the store keeps it.
    fn from(filename: FileName) -> Self {
        StoredText::from(filename.0)
    }
}

#[cfg(test)]
mod tests;
