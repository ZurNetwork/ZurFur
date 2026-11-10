use std::str::FromStr;

use super::NodeSegmentError;

/// The most bytes a node segment may hold: atproto's limit for a DID, the longest key.
pub const MAX_SEGMENT_BYTES: usize = 2048;

/// A node's path segment, its key under its parent.
///
/// Parsed by hand because the parse is the rule: 1 to [`MAX_SEGMENT_BYTES`] bytes of
/// `[A-Za-z0-9._:%~-]`, never `.` or `..`. It orders bytewise.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::Display, derive_more::AsRef,
)]
#[as_ref(str)]
pub struct NodeSegment(String);

/// Whether `byte` may appear in a segment.
fn allowed(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'%' | b'~' | b'-')
}

/// Checks `text` against the segment rule, before anything is copied.
fn check(text: &str) -> Result<(), NodeSegmentError> {
    if text.is_empty() {
        return Err(NodeSegmentError::Empty);
    }
    if text.len() > MAX_SEGMENT_BYTES {
        return Err(NodeSegmentError::TooLong);
    }
    if text == "." || text == ".." {
        return Err(NodeSegmentError::DotSegment);
    }
    if !text.bytes().all(allowed) {
        return Err(NodeSegmentError::Disallowed);
    }
    Ok(())
}

impl TryFrom<String> for NodeSegment {
    type Error = NodeSegmentError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        check(&text)?;
        Ok(Self(text))
    }
}

impl TryFrom<&str> for NodeSegment {
    type Error = NodeSegmentError;

    fn try_from(text: &str) -> Result<Self, Self::Error> {
        Self::try_from(text.to_owned())
    }
}

impl FromStr for NodeSegment {
    type Err = NodeSegmentError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        Self::try_from(text)
    }
}

#[cfg(test)]
mod tests;
