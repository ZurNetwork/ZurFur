use std::str::FromStr;

/// The most bytes a segment may hold: atproto's hard limit for a DID, the longest key.
pub const MAX_SEGMENT_BYTES: usize = 2048;

/// A node's key: its path segment, unique within its real parent.
///
/// Parsed by hand because the parse is the rule: 1 to [`MAX_SEGMENT_BYTES`] bytes of
/// `[A-Za-z0-9._:%~-]`, never `.` or `..`.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Display, derive_more::AsRef,
)]
#[as_ref(str)]
pub struct Segment(String);

/// Why a piece of text is not a [`Segment`]. Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SegmentError {
    /// The piece is empty.
    #[error("a path segment is empty")]
    Empty,
    /// The piece is longer than [`MAX_SEGMENT_BYTES`].
    #[error("a path segment is longer than {MAX_SEGMENT_BYTES} bytes")]
    TooLong,
    /// The piece is `.` or `..`.
    #[error("a path segment is `.` or `..`")]
    DotSegment,
    /// The piece holds a byte outside `[A-Za-z0-9._:%~-]`.
    #[error("a path segment holds a character outside its allowlist")]
    Disallowed,
}

/// Whether `byte` may appear in a segment.
fn allowed(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'%' | b'~' | b'-')
}

/// Checks `text` against the segment rule, before anything is copied.
fn check(text: &str) -> Result<(), SegmentError> {
    if text.is_empty() {
        return Err(SegmentError::Empty);
    }
    if text.len() > MAX_SEGMENT_BYTES {
        return Err(SegmentError::TooLong);
    }
    if text == "." || text == ".." {
        return Err(SegmentError::DotSegment);
    }
    if !text.bytes().all(allowed) {
        return Err(SegmentError::Disallowed);
    }
    Ok(())
}

impl TryFrom<String> for Segment {
    type Error = SegmentError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        check(&text)?;
        Ok(Self(text))
    }
}

impl TryFrom<&str> for Segment {
    type Error = SegmentError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        check(text)?;
        Ok(Self(text.to_owned()))
    }
}

impl FromStr for Segment {
    type Err = SegmentError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text)
    }
}

#[cfg(test)]
mod tests;
