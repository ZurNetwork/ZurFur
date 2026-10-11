use crate::{Level, NodeName, RealPath, Segment};

/// A node's type: an opaque key that only the host interprets.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, derive_more::From, derive_more::Display, derive_more::AsRef,
)]
#[as_ref(str)]
pub struct Type(String);

/// The fixed fields Atta keeps on every node, the same for every type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata {
    /// Its segment in its real parent.
    pub key: Segment,
    /// Its real parent's path; `None` only for the global root.
    pub parent: Option<RealPath>,
    /// Its type.
    pub typ: Type,
    /// Its own level, before any viewer's standing or grant.
    pub level: Level,
    /// Its display name.
    pub name: NodeName,
    /// A symlink's target; `None` for every other node.
    pub symlink_target: Option<RealPath>,
}
