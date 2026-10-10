use crate::{Level, Metadata, RealPath, Segment, Type};

/// Who is reading: an opaque value Atta passes to every host call and never inspects.
#[derive(Debug, Clone, PartialEq, Eq, Hash, derive_more::From, derive_more::AsRef)]
#[as_ref(str)]
pub struct Viewer(String);

/// Which relationship of the viewer's own a [`Lift`] stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Relationship {
    /// Standing at the node, such as being its user, a member or a participant.
    Standing,
    /// A grant given to the viewer for the node.
    Grant,
}

/// How far down a [`Lift`] reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// The node alone.
    Node,
    /// The node and its whole real subtree, never across a mount or a link.
    Subtree,
}

/// A level the viewer holds at a node through a relationship of their own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Lift {
    /// The level it lifts the viewer to.
    pub level: Level,
    /// How far down it reaches.
    pub scope: Scope,
    /// The relationship it stands for.
    pub relationship: Relationship,
}

/// A node as the host answers for it: its metadata, and the viewer's lifts at it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// Its metadata.
    pub metadata: Metadata,
    /// The viewer's lifts at this node; lifts at its ancestors are asked for at the ancestors.
    pub lifts: Vec<Lift>,
}

/// One of the viewer's mounts in a folder: a segment that grafts another node into it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mount {
    /// Its segment in the folder.
    pub segment: Segment,
    /// The real path of the node it grafts in.
    pub target: RealPath,
}

/// Whether a type's nodes hold entries or content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// It holds entries and can be listed.
    Directory,
    /// It holds content.
    File,
}

/// What opened a node for the viewer, or showed it as a card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Via {
    /// The node's own level alone.
    OwnLevel,
    /// The viewer's standing, at the node or at a real ancestor.
    Standing,
    /// The viewer's own grant.
    Grant,
}

/// The viewer's resolved access to a node, and what gave it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Access {
    /// The viewer's level at the node: [`Level::Listed`] for a card, [`Level::Public`] to open.
    pub level: Level,
    /// What gave that level; a relationship of the viewer's own wins over the own level.
    pub via: Via,
}

/// Why Atta asks the host to admit a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Reach {
    /// The node is an entry of a listing.
    Listing,
    /// The node is a step of a typed path, the last one included.
    Path,
}

/// The host's answer to [`Host::admit`]; a refusal carries no reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Admission {
    /// Keep the node.
    Admit,
    /// Leave the node out: dropped from a listing, the one not-found on a path.
    Refuse,
}

/// A failure inside the host, such as a store that can't be reached; never a miss.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct HostError(#[from] Box<dyn std::error::Error + Send + Sync>);

/// How Atta reaches the application that embeds it: storage, standing and types.
///
/// Every answer is in Atta's own values. A node the viewer may not know of is still answered
/// for; Atta decides what the viewer sees.
#[async_trait::async_trait]
pub trait Host: Send + Sync {
    /// The node at `key` in the real directory `parent`, with the viewer's lifts there.
    async fn child(
        &self,
        viewer: &Viewer,
        parent: &RealPath,
        key: &Segment,
    ) -> Result<Option<Found>, HostError>;

    /// Every node whose real parent is `parent`, each with the viewer's lifts there.
    async fn children(&self, viewer: &Viewer, parent: &RealPath) -> Result<Vec<Found>, HostError>;

    /// The viewer's own mounts in the real directory `folder`.
    async fn mounts(&self, viewer: &Viewer, folder: &RealPath) -> Result<Vec<Mount>, HostError>;

    /// Whether nodes of `typ` are directories or files.
    fn kind(&self, typ: &Type) -> Kind;

    /// Whether to keep a node the viewer can see, after Atta has resolved the viewer's access.
    ///
    /// Answers from what `child` and `children` already loaded for this request, with no store
    /// call, so a node refused on a path costs what an absent one does.
    async fn admit(
        &self,
        viewer: &Viewer,
        path: &RealPath,
        node: &Metadata,
        access: Access,
        reach: Reach,
    ) -> Result<Admission, HostError>;
}
