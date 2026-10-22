use crate::{Level, RealPath, Segment};

/// A node's type: an opaque key that only the host interprets.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, derive_more::From, derive_more::Display, derive_more::AsRef,
)]
#[as_ref(str)]
pub struct Type(String);

/// A node's display name, as the host checked it against its own name rule. Atta holds no
/// name rule of its own, so it shows the text exactly as the host gave it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::Display, derive_more::AsRef)]
#[as_ref(str)]
pub struct DisplayName(String);

impl DisplayName {
    /// Wraps text the host already checked. Inherent, not `From`, so the host can ban every
    /// call but its one construction site by this function's path.
    pub fn host_checked(text: String) -> Self {
        Self(text)
    }
}

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
    pub name: DisplayName,
    /// A symlink's target; `None` for every other node.
    pub symlink_target: Option<RealPath>,
}
