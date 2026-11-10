/// Error source of a node create whose segment another node already holds under
/// the same parent, or at the top when it has none. Carries no part of the segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a node already holds this segment there")]
pub struct SegmentTaken;
