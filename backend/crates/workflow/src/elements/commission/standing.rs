use kernel::elements::{level::Level, node::NodeId};

use super::CommissionRole;

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

/// What the locked standing read answers: the standing, and the node the commission points
/// at, read under the same lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockedStanding {
    /// The viewer's standing on the commission.
    pub standing: CommissionStanding,
    /// The node the commission points at.
    pub node: NodeId,
}
