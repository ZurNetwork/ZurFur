use std::fmt;

use super::{NodeId, NodeSegment, NodeType};
use crate::elements::{level::Level, text::StoredText};

/// A node as the store writes it. The caller mints its id.
#[derive(Clone, PartialEq, Eq)]
pub struct NewNode {
    /// Its key, app-private.
    pub id: NodeId,
    /// The node it sits under, or `None` for a node at the top.
    pub parent: Option<NodeId>,
    /// Its path segment, unique under its parent.
    pub segment: NodeSegment,
    /// What it is.
    pub node_type: NodeType,
    /// How visible it is on its own.
    pub level: Level,
    /// What a person reads, as the store will hold it.
    pub name: StoredText,
}

/// A node as the store holds it. Its name is loaded without re-checking.
#[derive(Clone, PartialEq, Eq)]
pub struct NodeRow {
    /// Its key, app-private.
    pub id: NodeId,
    /// The node it sits under, or `None` for a node at the top.
    pub parent: Option<NodeId>,
    /// Its path segment, unique under its parent.
    pub segment: NodeSegment,
    /// What it is.
    pub node_type: NodeType,
    /// How visible it is on its own.
    pub level: Level,
    /// What a person reads, as stored.
    pub name: StoredText,
}

impl From<NewNode> for NodeRow {
    /// The new node as it reads back once stored.
    fn from(node: NewNode) -> Self {
        let NewNode {
            id,
            parent,
            segment,
            node_type,
            level,
            name,
        } = node;
        Self {
            id,
            parent,
            segment,
            node_type,
            level,
            name,
        }
    }
}

/// Hand-written so the segment and the name never reach a log: only the keys, the
/// type and the level print.
impl fmt::Debug for NewNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewNode")
            .field("id", &self.id)
            .field("parent", &self.parent)
            .field("node_type", &self.node_type)
            .field("level", &self.level)
            .finish_non_exhaustive()
    }
}

/// Hand-written so the segment and the name never reach a log: only the keys, the
/// type and the level print.
impl fmt::Debug for NodeRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NodeRow")
            .field("id", &self.id)
            .field("parent", &self.parent)
            .field("node_type", &self.node_type)
            .field("level", &self.level)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
