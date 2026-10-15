use crate::den::SoftDelete;

/// Whether a node the host answered for is live or soft-deleted, as its arm read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum NodeState {
    /// Not soft-deleted.
    Live,
    /// An archived commission.
    Archived,
    /// A deactivated Account, and whether the viewer is its Owner.
    Deactivated {
        /// Whether the viewer is the Account's Owner.
        viewer_is_owner: bool,
    },
}

impl NodeState {
    /// The word an open node in this state is marked with; `None` when live.
    pub(crate) fn soft_delete(self) -> Option<SoftDelete> {
        match self {
            NodeState::Live => None,
            NodeState::Archived => Some(SoftDelete::Archived),
            NodeState::Deactivated { .. } => Some(SoftDelete::Deactivated),
        }
    }
}
