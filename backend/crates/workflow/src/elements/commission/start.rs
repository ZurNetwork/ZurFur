use kernel::datetime::DateTimeUtc;

use super::{ChannelPointer, DeadlineStatus, DirectionStatus, LifecycleStep, StoredDispute};

/// The state every new commission starts in, beside what its create chose. A store
/// writes [`CommissionStart::default`] as it is; no create chooses any of it.
#[derive(Debug, Clone, PartialEq)]
pub struct CommissionStart {
    /// The lifecycle step it starts at.
    pub lifecycle_step: LifecycleStep,
    /// The dispute it starts with.
    pub dispute: Option<StoredDispute>,
    /// The direction-axis Status it starts with.
    pub direction_status: Option<DirectionStatus>,
    /// The stored Delayed flag it starts with.
    pub delayed_flag: Option<DeadlineStatus>,
    /// The linked-channel pointer it starts with.
    pub linked_channel: Option<ChannelPointer>,
    /// The archive time it starts with.
    pub archived_at: Option<DateTimeUtc>,
}

impl Default for CommissionStart {
    /// A new commission starts Draft and undisputed, with no statuses (no Delayed flag), no
    /// linked channel, and live.
    fn default() -> Self {
        Self {
            lifecycle_step: LifecycleStep::Draft,
            dispute: None,
            direction_status: None,
            delayed_flag: None,
            linked_channel: None,
            archived_at: None,
        }
    }
}

#[cfg(test)]
mod tests;
