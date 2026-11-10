use kernel::elements::{level::Level, node::NodeId};

use super::{CommissionRole, CommissionSummary};

/// What one standing read answers about a viewer on one commission: the viewer's
/// role, the commission's own level and whether it is archived. Nothing else of the
/// commission crosses this read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommissionStanding {
    /// The viewer's role, or `None` for someone who holds none.
    pub role: Option<CommissionRole>,
    /// The level of the commission's node.
    pub level: Level,
    /// Whether it carries an archive time; the time itself isn't read.
    pub archived: bool,
}

impl CommissionStanding {
    /// Whether the viewer may be shown the commission: always when they hold a role, else only
    /// when it is live and its level is Listed or Public. An unreadable level reads as Private.
    pub fn is_shown(&self) -> bool {
        if self.role.is_some() {
            return true;
        }
        let open_level = matches!(self.level, Level::Listed | Level::Public);
        !self.archived && open_level
    }
}

/// A commission's summary as one viewer reads it, beside the role that viewer holds on it.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewedSummary {
    /// The commission's summary.
    pub summary: CommissionSummary,
    /// The viewer's role, or `None` for someone who holds none.
    pub role: Option<CommissionRole>,
}

impl From<&ViewedSummary> for CommissionStanding {
    /// The viewer's standing as the viewed summary carries it: the role, the summary's level
    /// and whether it is archived.
    fn from(viewed: &ViewedSummary) -> Self {
        Self {
            role: viewed.role,
            level: viewed.summary.visibility,
            archived: viewed.summary.archived,
        }
    }
}

/// What the locked standing read answers: the standing, and the node the commission points
/// at, read under the same lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockedStanding {
    /// The viewer's standing on the commission.
    pub standing: CommissionStanding,
    /// The node the commission points at.
    pub node: NodeId,
}

/// What the system's locked read answers, with no viewer: the node the commission points at
/// and that node's level, read under the commission row's lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockedNode {
    /// The node the commission points at.
    pub node: NodeId,
    /// That node's level.
    pub level: Level,
}

#[cfg(test)]
mod tests;
