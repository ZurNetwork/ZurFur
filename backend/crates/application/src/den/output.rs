use crate::den::{
    DenName, DenSegment, DenThing, DenType, NodeKind, NodeLevel, PageToken, SoftDelete,
};

/// One node as the Den shows it to its viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenNode {
    /// Its Den path from the viewer's root; empty for the root and on a card.
    pub segments: Vec<DenSegment>,
    /// Its display name, from what it holds.
    pub name: DenName,
    /// Its type.
    pub den_type: DenType,
    /// Whether it is one of the viewer's mounts rather than a real child of its folder.
    pub mount: bool,
    /// What the viewer may do with it.
    pub access: DenAccess,
}

/// The viewer's access to a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenAccess {
    /// The viewer can open it.
    Open {
        /// Whether it holds entries or content; a mount shows its target's.
        kind: NodeKind,
        /// Its own level; a mount shows its target's.
        own_level: NodeLevel,
        /// Whether this view doesn't show its content yet, fixed by its type.
        content_not_shown: bool,
        /// Its soft-deleted state; `None` when live.
        soft_delete: Option<SoftDelete>,
        /// The thing it stands for, an Account or a commission; a mount carries its target's.
        thing: Option<DenThing>,
    },
    /// The viewer may know it exists but can't open it.
    Card,
}

/// One folder above a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenCrumb {
    /// Its Den path from the viewer's root; empty for the root.
    pub segments: Vec<DenSegment>,
    /// Its display name.
    pub name: DenName,
    /// Its type; a mount shows its target's.
    pub den_type: DenType,
}

/// One page of an open directory's entries, after everything the viewer can't see is left out.
/// The default is an empty page with no next page.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DenListing {
    /// The entries, in the order they are shown.
    pub entries: Vec<DenNode>,
    /// Where the next page starts; `None` on the last page.
    pub next_page: Option<PageToken>,
}

/// What an open node holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DenContent {
    /// One page of a directory's entries.
    Listing(DenListing),
    /// A file; this read serves no file content yet.
    File,
}
