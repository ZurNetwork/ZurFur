use super::{
    ChannelPointer, CommissionId, CommissionTitle, DeadlineStatus, DirectionStatus, LifecycleStep,
    Visibility,
};
use crate::datetime::DateTimeUtc;
use crate::elements::maturity::Maturity;
use crate::elements::user::UserId;

/// A created commission and its fixed metadata. Build one with
/// [`Commission::create`]; it holds no participant list or composition, only the
/// always-present envelope.
#[derive(Debug)]
pub struct Commission {
    /// The app-private id (UUIDv7, so it sorts by creation time).
    pub id: CommissionId,
    /// The commission's Title — always present, validated non-empty.
    pub title: CommissionTitle,
    /// The User who created the commission and owns it.
    pub owner_id: UserId,
    /// The single lifecycle state the commission is in; a fresh one is
    /// [`LifecycleStep::Draft`].
    pub lifecycle_step: LifecycleStep,
    /// Who may see the commission; a fresh one is [`Visibility::Private`].
    pub visibility: Visibility,
    /// The deadline, or `None` when the commission carries none.
    pub deadline: Option<DateTimeUtc>,
    /// The commission's maturity posture. `None` at birth; a rating becomes
    /// required at the widening gate and, once set, replace-only — no path
    /// clears it back to `None`.
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
    /// Create a commission owned by `owner`, born in [`LifecycleStep::Draft`]
    /// with a fresh UUIDv7 id, no maturity and no statuses. Infallible — the
    /// title arrives already validated, and authority is the caller's concern.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{Commission, CommissionTitle, LifecycleStep}, did::Did, user::UserId,
    /// };
    ///
    /// let owner = UserId::from(Did::from("did:plc:alice".to_string()));
    /// let title = "A ref sheet".parse::<CommissionTitle>().unwrap();
    /// let c = Commission::create(title, owner.clone(), Utc::now(), None);
    /// assert_eq!(c.owner_id, owner);                             // the creator owns it
    /// assert!(matches!(c.lifecycle_step, LifecycleStep::Draft)); // born in Draft
    /// assert_eq!(c.title.as_str(), "A ref sheet");
    /// assert!(c.maturity.is_none()); // born unrated: a rating gates widening, not birth
    /// ```
    pub fn create(
        title: CommissionTitle,
        owner: UserId,
        now: DateTimeUtc,
        deadline: Option<DateTimeUtc>,
    ) -> Self {
        Self {
            id: CommissionId::from(uuid::Uuid::now_v7()),
            title,
            owner_id: owner,
            lifecycle_step: LifecycleStep::Draft,
            created_at: now,
            visibility: Visibility::Private,
            deadline,
            maturity: None,
            direction_status: None,
            deadline_status: None,
            linked_channel: None,
            archived_at: None,
        }
    }

    pub fn is_archived(&self) -> bool {
        self.archived_at.is_none()
    }

    pub fn is_owned_by(&self, user_id: &UserId) -> bool {
        self.owner_id == *user_id
    }
}

#[cfg(test)]
mod tests;
