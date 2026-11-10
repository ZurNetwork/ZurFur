use std::fmt;

use crate::{DisplayName, Level, RealPath, Segment, Type};

/// An act's word: an opaque key that only the host interprets, like [`Type`].
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, derive_more::From, derive_more::Display, derive_more::AsRef,
)]
#[as_ref(str)]
pub struct ActName(String);

/// A node an act touches, as the host found it.
#[derive(Clone, PartialEq, Eq)]
pub struct Placed {
    /// Where it really lives.
    pub path: RealPath,
    /// Its type.
    pub typ: Type,
    /// Its own level.
    pub level: Level,
}

/// A node an act makes. Its key is born exactly as written; Atta never decodes it.
#[derive(Clone, PartialEq, Eq)]
pub struct Born {
    /// Its segment in its parent.
    pub key: Segment,
    /// Its type.
    pub typ: Type,
    /// Its display name, as the host checked it.
    pub name: DisplayName,
    /// The level its creator picked; `None` means Private.
    pub level: Option<Level>,
}

/// What an act does to the tree.
#[derive(Clone, PartialEq, Eq)]
pub enum Shape {
    /// Makes `node` in the directory `parent`, holding `entries`.
    Create {
        /// The directory it is made in.
        parent: Placed,
        /// The node it makes.
        node: Born,
        /// The entries made inside the node.
        entries: Vec<Born>,
    },
    /// Gives `node` a new display name.
    Rename {
        /// The node it renames.
        node: Placed,
        /// Its new display name, as the host checked it.
        name: DisplayName,
    },
    /// Changes `node`'s content.
    Change {
        /// The node it changes.
        node: Placed,
    },
}

impl Shape {
    /// The depth of the node it acts on or makes: a top-level directory is at depth 1.
    pub fn depth(&self) -> usize {
        match self {
            Self::Create { parent, .. } => parent.path.depth() + 1,
            Self::Rename { node, .. } | Self::Change { node } => node.path.depth(),
        }
    }
}

/// A named act on the tree: its word, the type it acts for, what it does, and a body only the
/// host reads.
#[derive(Clone, PartialEq, Eq)]
pub struct Act<B> {
    /// Its word.
    pub name: ActName,
    /// The type of the node it acts on or makes.
    pub typ: Type,
    /// What it does to the tree.
    pub shape: Shape,
    /// What the host needs to apply it; opaque to Atta.
    pub body: B,
}

impl fmt::Debug for Placed {
    /// Its depth, type and level; never its path.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Placed")
            .field("depth", &self.path.depth())
            .field("typ", &self.typ)
            .field("level", &self.level)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Born {
    /// Its type and picked level; never its key or name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Born")
            .field("typ", &self.typ)
            .field("level", &self.level)
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for Shape {
    /// Its variant and the nodes' [`Debug`](fmt::Debug)s; never a new name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Create {
                parent,
                node,
                entries,
            } => formatter
                .debug_struct("Create")
                .field("parent", parent)
                .field("node", node)
                .field("entries", entries)
                .finish(),
            Self::Rename { node, .. } => formatter
                .debug_struct("Rename")
                .field("node", node)
                .finish_non_exhaustive(),
            Self::Change { node } => formatter
                .debug_struct("Change")
                .field("node", node)
                .finish(),
        }
    }
}

impl<B> fmt::Debug for Act<B> {
    /// Its word, type and shape; never its body.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Act")
            .field("name", &self.name)
            .field("typ", &self.typ)
            .field("shape", &self.shape)
            .finish_non_exhaustive()
    }
}
