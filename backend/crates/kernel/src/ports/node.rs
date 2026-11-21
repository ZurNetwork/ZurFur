use async_trait::async_trait;

use crate::elements::node::{NewNode, NodeId, NodeRow, NodeSegment, NodeType};
use crate::elements::text::StoredText;

/// The read surface of the Den's nodes — pool-backed and non-transactional; every
/// write lives on [`NodeWrites`].
#[async_trait]
pub trait NodeStore: Send + Sync {
    /// The top-level node of `node_type` holding `segment`, or `None`.
    async fn at_top(
        &self,
        node_type: &NodeType,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>>;

    /// The node under `parent` holding `segment`, or `None`.
    async fn child(
        &self,
        parent: &NodeId,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>>;

    /// Every node directly under `parent`, ordered by segment bytewise; empty when
    /// `parent` holds none or doesn't exist.
    async fn children(&self, parent: &NodeId) -> anyhow::Result<Vec<NodeRow>>;
}

/// The read side of a node unit of work — [`NodeStore`]'s lookups on the unit's own
/// connection, seeing the unit's writes.
#[async_trait]
pub trait NodeReads: Send {
    /// The top-level node of `node_type` holding `segment`, or `None`; as [`NodeStore::at_top`].
    async fn at_top(
        &mut self,
        node_type: &NodeType,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>>;

    /// The node under `parent` holding `segment`, or `None`; as [`NodeStore::child`].
    async fn child(
        &mut self,
        parent: &NodeId,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>>;

    /// Every node directly under `parent`, bytewise by segment; as [`NodeStore::children`].
    async fn children(&mut self, parent: &NodeId) -> anyhow::Result<Vec<NodeRow>>;
}

/// The write surface of the Den's nodes — reachable only on an open `UnitOfWork`.
///
/// A refused node call ends the unit: every later node call fails, and the unit's
/// `commit` returns an error and keeps nothing.
#[async_trait]
pub trait NodeWrites: Send {
    /// Store a new node. A segment its parent (or the top) already holds fails with
    /// [`SegmentTaken`](super::SegmentTaken) as the error source; a node that doesn't
    /// fit its parent, an empty name, a missing parent or a taken id fails without it.
    async fn create(&mut self, node: &NewNode) -> anyhow::Result<()>;

    /// Replace the name of node `id`; `false` when there is no such node.
    async fn set_name(&mut self, id: &NodeId, name: &StoredText) -> anyhow::Result<bool>;

    /// Delete node `id` and every node under it; `false` when there is no such node.
    /// A fixed entry is refused while its commission node lives.
    async fn delete(&mut self, id: &NodeId) -> anyhow::Result<bool>;
}

/// A node unit of work: [`NodeReads`] + [`NodeWrites`] on one connection. Vended by
/// `UnitOfWork::nodes`.
pub trait NodeRepo: NodeReads + NodeWrites {}

impl<T: NodeReads + NodeWrites> NodeRepo for T {}
