use super::{
    ChannelPointer, CommissionId, CommissionTitle, DeadlineStatus, DirectionStatus, LifecycleStep,
    Visibility,
};
use crate::{
    datetime::DateTimeUtc,
    elements::{maturity::Maturity, text::StoredText, user::UserId},
};

/// A freshly created commission, ready to persist through
/// [`CommissionWrites::create`](crate::ports::CommissionWrites::create). Built by
/// [`Commission::create`](super::Commission::create). Its title is a checked
/// [`CommissionTitle`], so no unchecked title can reach the write.
///
/// ```compile_fail,E0308
/// use domain::elements::{commission::NewCommission, text::StoredText};
///
/// fn store_unchecked(new: &mut NewCommission) {
///     new.title = StoredText::from("   ".to_owned()); // refused: not a CommissionTitle
/// }
/// ```
#[derive(Debug)]
pub struct NewCommission {
    /// The app-private id (UUIDv7, so it sorts by creation time).
    pub id: CommissionId,
    /// The checked Title.
    pub title: CommissionTitle,
    /// The User who creates the commission and owns it.
    pub owner_id: UserId,
    /// The lifecycle state it is born in.
    pub lifecycle_step: LifecycleStep,
    /// Who may see it.
    pub visibility: Visibility,
    /// The deadline, or `None`.
    pub deadline: Option<DateTimeUtc>,
    /// The maturity posture, or `None` while unrated.
    pub maturity: Option<Maturity>,
    /// The direction-axis Status, or `None`.
    pub direction_status: Option<DirectionStatus>,
    /// The deadline-axis Status, or `None`.
    pub deadline_status: Option<DeadlineStatus>,
    /// The external linked-channel pointer, or `None`.
    pub linked_channel: Option<ChannelPointer>,
    /// When it was archived, or `None` while active.
    pub archived_at: Option<DateTimeUtc>,
    /// When it was created.
    pub created_at: DateTimeUtc,
}

/// One commission as a listing reads it: its id, stored title, own visibility
/// and archive time. Nothing else of the commission crosses this read.
#[derive(Debug, Clone, PartialEq)]
pub struct CommissionSummary {
    /// The commission's id.
    pub id: CommissionId,
    /// The title as stored, not re-checked on load.
    pub title: StoredText,
    /// The commission's own visibility.
    pub visibility: Visibility,
    /// When it was archived, or `None` while active.
    pub archived_at: Option<DateTimeUtc>,
}

impl CommissionSummary {
    /// Whether the commission is archived: true once it carries an archive time.
    pub fn is_archived(&self) -> bool {
        self.archived_at.is_some()
    }
}

#[cfg(test)]
mod tests;
