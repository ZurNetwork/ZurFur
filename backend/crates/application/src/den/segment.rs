use atta::Segment;

/// One piece of a Den path, sent back to the client unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct DenSegment(Segment);

impl DenSegment {
    /// Wraps a segment the engine answered with; inherent, because a public `From` would put
    /// the engine's type in this crate's API.
    pub(crate) fn new(segment: Segment) -> Self {
        Self(segment)
    }
}
