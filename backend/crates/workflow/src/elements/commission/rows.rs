use super::CommissionId;
use identity::elements::user::UserId;
use kernel::{
    datetime::DateTimeUtc,
    elements::{level::Level, maturity::Maturity, node::NodeId, text::StoredText},
};

/// What a create may choose of a new commission: its maturity and its deadline. The rest
/// of its starting state is [`CommissionStart`](super::CommissionStart)'s, so a create
/// can't choose that.
///
/// ```compile_fail,E0560
/// use workflow::elements::commission::{LifecycleStep, NewCommission};
///
/// let chosen = NewCommission {
///     maturity: None,
///     deadline: None,
///     lifecycle_step: LifecycleStep::Active, // refused: a create can't choose its lifecycle
/// };
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCommission {
    /// The maturity posture, or `None` while unrated.
    pub maturity: Option<Maturity>,
    /// The deadline, or `None`.
    pub deadline: Option<DateTimeUtc>,
}

/// What the den's write gate supplies to a create, beside its capability: the commission's
/// node, its owner and when it was made. The create itself chooses none of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionBirth {
    /// The Den node the commission stands at.
    pub node: NodeId,
    /// The User who creates the commission and owns it.
    pub owner_id: UserId,
    /// When it was created.
    pub created_at: DateTimeUtc,
}

/// One commission as a listing reads it: its id, its node's name and level, and whether
/// it is archived. Nothing else of the commission crosses this read.
#[derive(Debug, Clone, PartialEq)]
pub struct CommissionSummary {
    /// The commission's id.
    pub id: CommissionId,
    /// Its node's name as stored, not re-checked on load.
    pub title: StoredText,
    /// Its node's level.
    pub visibility: Level,
    /// Whether it carries an archive time; the time itself isn't read.
    pub archived: bool,
}

impl CommissionSummary {
    /// Whether the commission is archived.
    pub fn is_archived(&self) -> bool {
        self.archived
    }
}

#[cfg(test)]
mod tests;
