use super::{ChannelPointer, CommissionId, DeadlineStatus, DirectionStatus, LifecycleStep};
use identity::elements::user::UserId;
use kernel::datetime::DateTimeUtc;
use kernel::elements::maturity::Maturity;

/// A stored commission and its fixed metadata, as a load returns it. A new one is
/// written from a [`NewCommission`](super::NewCommission) and the gate's
/// [`CommissionBirth`](super::CommissionBirth); it holds no participant list or
/// composition, only the always-present envelope. Its name and level are its
/// node's, read through a [`CommissionSummary`](super::CommissionSummary).
#[derive(Debug)]
pub struct Commission {
    /// The app-private id (UUIDv7, so it sorts by creation time).
    pub id: CommissionId,
    /// The User who created the commission and owns it.
    pub owner_id: UserId,
    /// The single lifecycle state the commission is in; a fresh one is at
    /// [`CommissionStart`](super::CommissionStart)'s.
    pub lifecycle_step: LifecycleStep,
    /// The deadline, or `None` when the commission carries none.
    pub deadline: Option<DateTimeUtc>,
    /// The commission's maturity posture, or `None` while unrated; a create may
    /// leave it unrated. A rating becomes required at the widening gate and,
    /// once set, replace-only — no path clears it back to `None`.
    pub maturity: Option<Maturity>,
    /// The direction-axis Status, or `None` when cleared. One nullable cell, so
    /// a set replaces; only an explicit Participant act moves it, never a
    /// content event.
    pub direction_status: Option<DirectionStatus>,
    /// The deadline-axis Status, or `None` while none is held. Holds a value
    /// only while [`deadline`](Commission::deadline) is set; see
    /// [`DeadlineStatus`] for who moves it.
    pub deadline_status: Option<DeadlineStatus>,
    /// The external linked-channel pointer — "where we talk" — or `None` while
    /// no channel is declared. Owner-set and changelog-recorded on set/clear.
    pub linked_channel: Option<ChannelPointer>,
    /// When the commission was archived — `None` while active. Owner-only in
    /// both directions and changelog-recorded; the record and its facts survive
    /// intact, and listing projections filter on this field.
    pub archived_at: Option<DateTimeUtc>,
    /// When the commission was created.
    pub created_at: DateTimeUtc,
}

impl Commission {
    /// Whether the commission is archived: true once it carries an archive time.
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }

    /// Whether `user_id` is the commission's owner.
    pub fn is_owned_by(&self, user_id: &UserId) -> bool {
        self.owner_id == *user_id
    }
}

#[cfg(test)]
mod tests;
