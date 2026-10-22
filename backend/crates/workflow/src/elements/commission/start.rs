use kernel::datetime::DateTimeUtc;

use super::{ChannelPointer, DeadlineStatus, DirectionStatus, LifecycleStep};

/// The state every new commission starts in, beside what its create chose. A store
/// writes [`CommissionStart::default`] as it is; no create chooses any of it.
#[derive(Debug, Clone, PartialEq)]
pub struct CommissionStart {
    /// The lifecycle step it starts at.
    pub lifecycle_step: LifecycleStep,
    /// The direction-axis Status it starts with.
    pub direction_status: Option<DirectionStatus>,
    /// The deadline-axis Status it starts with.
    pub deadline_status: Option<DeadlineStatus>,
    /// The linked-channel pointer it starts with.
    pub linked_channel: Option<ChannelPointer>,
    /// The archive time it starts with.
    pub archived_at: Option<DateTimeUtc>,
}

impl Default for CommissionStart {
    /// A new commission starts Draft, with no statuses, no linked channel, and live.
    fn default() -> Self {
        Self {
            lifecycle_step: LifecycleStep::Draft,
            direction_status: None,
            deadline_status: None,
            linked_channel: None,
            archived_at: None,
        }
    }
}

#[cfg(test)]
mod tests;
