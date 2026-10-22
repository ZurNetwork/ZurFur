//! In-memory fakes for the node ports: a staged map of nodes that mirrors the
//! PostgreSQL table's rules — unique segments under a parent and at the top, a
//! parent that exists, the shape rules, the fixed-entry delete refusal and the
//! cascade — ordered bytewise by segment. A node refusal spends the unit.

use std::collections::HashMap;

use async_trait::async_trait;
use domain::elements::node::{NewNode, NodeId, NodeRow, NodeSegment, NodeType};
use domain::elements::text::StoredText;
use domain::ports::{NodeReads, NodeStore, NodeWrites, SegmentTaken};

use crate::MemBackend;

/// The prefix every fixed entry's word follows in its type.
const ENTRY_PREFIX: &str = "commission.";

/// In-memory [`NodeStore`] read surface over the shared [`MemBackend`].
pub struct MemNodeStore(pub(crate) MemBackend);

/// In-memory node view, vended by [`MemUnitOfWork::nodes`](crate::MemUnitOfWork)
/// over the unit's staging snapshot. A refused call marks the unit spent.
pub struct MemNodeWrites<'a> {
    /// The unit's staging snapshot, where the writes land.
    pub(crate) staged: MemBackend,
    /// The unit's spent mark: set by any refused node call, read by its commit.
    pub(crate) spent: &'a mut bool,
}

/// The fixed entry's word when `node_type` is one of the six entries.
pub(crate) fn entry_word(node_type: NodeType) -> Option<&'static str> {
    let word: &'static str = node_type.into();
    word.strip_prefix(ENTRY_PREFIX)
}

/// The top-level node of `node_type` holding `segment` in `nodes`.
fn at_top(
    nodes: &HashMap<NodeId, NodeRow>,
    node_type: &NodeType,
    segment: &NodeSegment,
) -> Option<NodeRow> {
    nodes
        .values()
        .find(|node| {
            node.parent.is_none() && node.node_type == *node_type && node.segment == *segment
        })
        .cloned()
}

/// The node under `parent` holding `segment` in `nodes`.
fn child(
    nodes: &HashMap<NodeId, NodeRow>,
    parent: &NodeId,
    segment: &NodeSegment,
) -> Option<NodeRow> {
    nodes
        .values()
        .find(|node| node.parent == Some(*parent) && node.segment == *segment)
        .cloned()
}

/// Every node directly under `parent` in `nodes`, bytewise by segment.
fn children(nodes: &HashMap<NodeId, NodeRow>, parent: &NodeId) -> Vec<NodeRow> {
    let mut listed: Vec<NodeRow> = nodes
        .values()
        .filter(|node| node.parent == Some(*parent))
        .cloned()
        .collect();
    listed.sort_by(|left, right| left.segment.cmp(&right.segment));
    listed
}

/// Why `node` can't be stored beside `nodes`, checked in the order PostgreSQL
/// checks it: the insert trigger, the row checks, then the keys.
fn refusal(nodes: &HashMap<NodeId, NodeRow>, node: &NewNode) -> Option<anyhow::Error> {
    let parent_type = node
        .parent
        .and_then(|parent| nodes.get(&parent))
        .map(|parent| parent.node_type);
    let entry = entry_word(node.node_type);
    if node.parent.is_some() {
        if node.node_type == NodeType::Slot && parent_type != Some(NodeType::CommissionSlots) {
            return Some(anyhow::anyhow!(
                "node does not fit its parent: a slot sits only under a commission.slots node"
            ));
        }
        if entry.is_some() && parent_type != Some(NodeType::Commission) {
            return Some(anyhow::anyhow!(
                "node does not fit its parent: an entry sits only under a commission node"
            ));
        }
    }
    if node.name.as_str().is_empty() {
        return Some(anyhow::anyhow!("a node's name is never empty"));
    }
    if node.parent.is_none() != (node.node_type == NodeType::Commission) {
        return Some(anyhow::anyhow!(
            "only a commission node sits at the top, and it always does"
        ));
    }
    if let Some(word) = entry
        && node.segment.as_ref() != word
    {
        return Some(anyhow::anyhow!("an entry's segment is its word"));
    }
    if nodes.contains_key(&node.id) {
        return Some(anyhow::anyhow!("a node already has this id"));
    }
    let taken = nodes
        .values()
        .any(|stored| stored.parent == node.parent && stored.segment == node.segment);
    if taken {
        return Some(anyhow::Error::new(SegmentTaken));
    }
    if let Some(parent) = node.parent
        && !nodes.contains_key(&parent)
    {
        return Some(anyhow::anyhow!("a node's parent exists"));
    }
    None
}

/// Every node under `root`, `root` included.
pub(crate) fn subtree(nodes: &HashMap<NodeId, NodeRow>, root: NodeId) -> Vec<NodeId> {
    let mut found = vec![root];
    let mut next = 0;
    while let Some(current) = found.get(next).copied() {
        let under = nodes
            .values()
            .filter(|node| node.parent == Some(current))
            .map(|node| node.id);
        found.extend(under);
        next += 1;
    }
    found
}

impl MemNodeWrites<'_> {
    /// Refuses a call on a unit a node refusal already spent.
    fn still_open(&self) -> anyhow::Result<()> {
        anyhow::ensure!(!*self.spent, "a refused node call ended this unit of work");
        Ok(())
    }

    /// Spends the unit and hands back `error`.
    fn spend(&mut self, error: anyhow::Error) -> anyhow::Error {
        *self.spent = true;
        error
    }
}

#[async_trait]
impl NodeStore for MemNodeStore {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn at_top(
        &self,
        node_type: &NodeType,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>> {
        let nodes = self
            .0
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        Ok(at_top(&nodes, node_type, segment))
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn child(
        &self,
        parent: &NodeId,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>> {
        let nodes = self
            .0
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        Ok(child(&nodes, parent, segment))
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn children(&self, parent: &NodeId) -> anyhow::Result<Vec<NodeRow>> {
        let nodes = self
            .0
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        Ok(children(&nodes, parent))
    }
}

#[async_trait]
impl NodeReads for MemNodeWrites<'_> {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn at_top(
        &mut self,
        node_type: &NodeType,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>> {
        self.still_open()?;
        let nodes = self
            .staged
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        Ok(at_top(&nodes, node_type, segment))
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn child(
        &mut self,
        parent: &NodeId,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>> {
        self.still_open()?;
        let nodes = self
            .staged
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        Ok(child(&nodes, parent, segment))
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn children(&mut self, parent: &NodeId) -> anyhow::Result<Vec<NodeRow>> {
        self.still_open()?;
        let nodes = self
            .staged
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        Ok(children(&nodes, parent))
    }
}

#[async_trait]
impl NodeWrites for MemNodeWrites<'_> {
    /// Stores the node unless a rule refuses it; only a taken segment is [`SegmentTaken`].
    #[tracing::instrument(level = "debug", skip_all)]
    async fn create(&mut self, node: &NewNode) -> anyhow::Result<()> {
        self.still_open()?;
        let staged = self.staged.clone();
        let mut nodes = staged
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        if let Some(error) = refusal(&nodes, node) {
            return Err(self.spend(error));
        }
        nodes.insert(node.id, NodeRow::from(node.clone()));
        Ok(())
    }

    /// Replaces the name; an empty name is refused, as the table's check refuses it.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn set_name(&mut self, id: &NodeId, name: &StoredText) -> anyhow::Result<bool> {
        self.still_open()?;
        let staged = self.staged.clone();
        let mut nodes = staged
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        let Some(node) = nodes.get_mut(id) else {
            return Ok(false);
        };
        if name.as_str().is_empty() {
            return Err(self.spend(anyhow::anyhow!("a node's name is never empty")));
        }
        node.name = name.clone();
        Ok(true)
    }

    /// Deletes the node and its subtree; a fixed entry whose parent lives is refused.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn delete(&mut self, id: &NodeId) -> anyhow::Result<bool> {
        self.still_open()?;
        let staged = self.staged.clone();
        let mut nodes = staged
            .nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned");
        let Some(node) = nodes.get(id) else {
            return Ok(false);
        };
        let parent_lives = node
            .parent
            .is_some_and(|parent| nodes.contains_key(&parent));
        if entry_word(node.node_type).is_some() && parent_lives {
            return Err(self.spend(anyhow::anyhow!(
                "a fixed entry leaves only with its commission"
            )));
        }
        for gone in subtree(&nodes, *id) {
            nodes.remove(&gone);
        }
        Ok(true)
    }
}

/// Node inspect helpers.
impl MemBackend {
    /// Every stored node, in unspecified order (inspect helper), so a test can see what a
    /// write left behind anywhere in the tree.
    pub fn all_nodes(&self) -> Vec<NodeRow> {
        self.nodes
            .lock()
            .expect("MemBackend nodes mutex poisoned")
            .values()
            .cloned()
            .collect()
    }
}
