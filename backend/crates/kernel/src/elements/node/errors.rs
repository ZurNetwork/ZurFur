use super::MAX_SEGMENT_BYTES;

/// Why a piece of text is not a [`NodeSegment`](super::NodeSegment). Carries no part of the text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NodeSegmentError {
    /// The piece is empty.
    #[error("a node segment is empty")]
    Empty,
    /// The piece is longer than [`MAX_SEGMENT_BYTES`].
    #[error("a node segment is longer than {MAX_SEGMENT_BYTES} bytes")]
    TooLong,
    /// The piece is `.` or `..`.
    #[error("a node segment is `.` or `..`")]
    DotSegment,
    /// The piece holds a byte outside `[A-Za-z0-9._:%~-]`.
    #[error("a node segment holds a character outside its allowlist")]
    Disallowed,
}
