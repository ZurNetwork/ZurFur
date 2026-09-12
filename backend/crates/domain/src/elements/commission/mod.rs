//! The [`Commission`] — the platform's basic unit of work and the aggregator of
//! everything done under it. (DESIGN 3276807)
//!
//! This module holds the envelope: id, title, owner, lifecycle, visibility,
//! deadline, maturity, statuses. A commission belongs to a User, never an
//! account, and survives account deletion. Its [`Visibility`] is the outermost
//! gate, applied before the composition's own [`effective_visibility`].

pub mod changelog;
pub mod element;
pub mod fact;
pub mod file;
pub mod markup;
pub mod positioning;
pub mod seat;
pub mod seat_invitation;
pub mod slot;

pub use changelog::{
    ChangelogEntry, ChangelogEntryKind, ChannelPointer, ChannelPointerError, NewChangelogEntry,
};
pub use element::{
    Band, CommissionComposition, CompositionLabelError, DeclaredTab, ElementId, ElementPayload,
    ElementRow, ElementType, LABEL_MAX_CHARS, NewElement, SKELETON, SurfaceAddress, SurfaceName,
    TabId, TabName, TabRow, VisibilityMode, declared_tabs, declares_surface, effective_visibility,
};
pub use fact::Fact;
pub use file::{CommissionFile, FileDownload, FileKey, FileMetadata, FileName, FileNameError};
pub use markup::{CommissionMarkup, Markup, MarkupError, MarkupKey, MarkupShape};
pub use positioning::GrantLevel;
pub use seat::{
    NewSeat, Seat, SeatKind, SeatKindError, SeatLink, SeatLinkError, SeatPrompt, SeatPromptError,
};
pub use seat_invitation::{SeatInvitation, SeatInvitationId};
use serde::{Deserialize, Serialize};
pub use slot::{NewSlot, Slot, SlotTitle, SlotTitleError};
use uuid::Uuid;

use std::ops::Deref;
use std::str::FromStr;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        id::{IdError, parse_uuid},
        maturity::Maturity,
        user::UserId,
    },
    string_builder::{StringBuilder, StringBuilderViolation},
};

/// The app-private key of a [`Commission`] (UUIDv7, so it sorts by creation
/// time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommissionId(uuid::Uuid);

impl CommissionId {
    /// Wraps an already-minted UUIDv7.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl From<Uuid> for CommissionId {
    fn from(id: Uuid) -> Self {
        Self(id)
    }
}

impl Deref for CommissionId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for CommissionId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_uuid(s).map(Self)
    }
}

/// A commission's Title: trimmed, and non-empty. No length cap yet.
///
/// ```
/// use domain::elements::commission::CommissionTitle;
///
/// let title = "  A ref sheet  ".parse::<CommissionTitle>().unwrap();
/// assert_eq!(title.as_str(), "A ref sheet"); // trimmed
///
/// assert!("   ".parse::<CommissionTitle>().is_err()); // empty after trim
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionTitle(String);

/// Why a string was rejected as a commission title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommissionTitleError {
    /// Empty once trimmed.
    Empty,
}

impl std::fmt::Display for CommissionTitleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommissionTitleError::Empty => write!(f, "commission title must not be empty"),
        }
    }
}

impl std::error::Error for CommissionTitleError {}

impl CommissionTitle {
    /// The validated, trimmed title as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for CommissionTitle {
    type Error = CommissionTitleError;

    /// Validate and wrap a title: trim, then reject an empty result.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
        StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => CommissionTitleError::Empty,
                StringBuilderViolation::TooLong { .. }
                | StringBuilderViolation::ControlCharacter => {
                    // Unreachable: this chain only applies trimmed().non_empty().
                    debug_assert!(
                        false,
                        "CommissionTitle's TryFrom chain only applies trimmed().non_empty()"
                    );
                    CommissionTitleError::Empty
                }
            })
    }
}

impl std::str::FromStr for CommissionTitle {
    type Err = CommissionTitleError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

impl AsRef<str> for CommissionTitle {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A created commission and its fixed metadata. Build one with
/// [`Commission::create`]; it holds no participant list or composition, only the
/// always-present envelope. (DESIGN 3276807)
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
    /// clears it back to `None`. (DD 29982722)
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
    /// intact, and listing projections filter on this field. (DD 3014657)
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
    /// let owner = UserId::new(Did::new("did:plc:alice".to_string()));
    /// let title = "A ref sheet".parse::<CommissionTitle>().unwrap();
    /// let c = Commission::create(title, owner.clone(), Utc::now(), None);
    /// assert_eq!(c.owner_id, owner);                             // the creator owns it
    /// assert!(matches!(c.lifecycle_step, LifecycleStep::Draft)); // born in Draft
    /// assert_eq!(c.title.as_str(), "A ref sheet");
    /// assert!(c.maturity.is_none()); // born unrated (ZMVP-31: rating gates widening, not birth)
    /// ```
    pub fn create(
        title: CommissionTitle,
        owner: UserId,
        now: DateTimeUtc,
        deadline: Option<DateTimeUtc>,
    ) -> Self {
        Self {
            id: CommissionId::new(uuid::Uuid::now_v7()),
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

/// The single lifecycle state a commission holds. Always exactly one, moved
/// explicitly by a participant and never by a system event.
#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleStep {
    /// Just created; no facts yet, so hard delete is possible.
    Draft,
    /// Part of the workload but not active
    Batched,
    /// Selected to be worked in the batch
    Active,
    /// Approved and closed
    Completed,
    /// Cancelled by one of the parties
    Cancelled,
    /// Disputed and requiring intervention
    Disputed,
}

impl LifecycleStep {
    /// Every state, in declaration order — the closed vocabulary.
    pub const ALL: &[LifecycleStep] = &[
        Self::Draft,
        Self::Batched,
        Self::Active,
        Self::Completed,
        Self::Cancelled,
        Self::Disputed,
    ];

    /// Whether this state is terminal — closed work, out of scope for the
    /// deadline sweeper. [`Disputed`](Self::Disputed) is *not* terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }

    /// The stable, lowercase token written to `commission.lifecycle`.
    /// Persisted — renaming a token is a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Batched => "batched",
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Disputed => "disputed",
        }
    }
}

/// Why a token failed to resolve to a [`LifecycleStep`]. Surfaced as an error,
/// never a silent default.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownLifecycleStep;

impl std::fmt::Display for UnknownLifecycleStep {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("token is not one of: draft, batched, active, completed, cancelled, disputed")
    }
}

impl std::error::Error for UnknownLifecycleStep {}

impl TryFrom<&str> for LifecycleStep {
    type Error = UnknownLifecycleStep;

    /// Resolve a stored token back to its step.
    fn try_from(token: &str) -> Result<Self, Self::Error> {
        Ok(match token {
            "draft" => Self::Draft,
            "batched" => Self::Batched,
            "active" => Self::Active,
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            "disputed" => Self::Disputed,
            _ => return Err(UnknownLifecycleStep),
        })
    }
}

/// The direction-axis Status a commission may carry — whose turn the work is
/// waiting on. Always set explicitly by a Participant, never by a content
/// event. One nullable column, so a set replaces and `None` means cleared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirectionStatus {
    /// The work waits on input from the client side.
    WaitingForInput,
    /// The work waits on an approval.
    WaitingForApproval,
    /// Changes were requested on what was delivered.
    ChangesRequested,
}

impl DirectionStatus {
    /// Every value, in declaration order — the closed three-value vocabulary.
    //FIXME: This is a disallowed pattern
    pub const ALL: &[DirectionStatus] = &[
        Self::WaitingForInput,
        Self::WaitingForApproval,
        Self::ChangesRequested,
    ];

    /// The stable, lowercase token written to `commission.direction_status`.
    /// Persisted — renaming a token is a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WaitingForInput => "waiting_for_input",
            Self::WaitingForApproval => "waiting_for_approval",
            Self::ChangesRequested => "changes_requested",
        }
    }
}

/// Why a token failed to resolve to a [`DirectionStatus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownDirectionStatus;

impl std::fmt::Display for UnknownDirectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "token is not one of: waiting_for_input, waiting_for_approval, changes_requested",
        )
    }
}

impl std::error::Error for UnknownDirectionStatus {}

impl TryFrom<&str> for DirectionStatus {
    type Error = UnknownDirectionStatus;

    /// Resolve a stored token back to its value.
    fn try_from(token: &str) -> Result<Self, Self::Error> {
        Ok(match token {
            "waiting_for_input" => Self::WaitingForInput,
            "waiting_for_approval" => Self::WaitingForApproval,
            "changes_requested" => Self::ChangesRequested,
            _ => return Err(UnknownDirectionStatus),
        })
    }
}

impl std::fmt::Display for DirectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WaitingForInput => write!(f, "waiting_for_input"),
            Self::WaitingForApproval => write!(f, "waiting_for_approval"),
            Self::ChangesRequested => write!(f, "changes_requested"),
        }
    }
}

/// The deadline-axis Status a commission may carry — how the work stands
/// against its deadline. One nullable cell, so a set replaces; a commission with
/// no deadline never carries one.
///
/// [`Delayed`](Self::Delayed) is a manual Participant flag; [`Late`](Self::Late)
/// is the system's word, never set by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeadlineStatus {
    /// The work is slipping — delays, but not yet lateness.
    Delayed,
    /// The deadline passed — system-set by the sweeper, never by hand.
    Late,
}

impl DeadlineStatus {
    /// Every value, in declaration order — the closed two-value vocabulary.
    pub const ALL: &[DeadlineStatus] = &[Self::Delayed, Self::Late];

    /// The stable, lowercase token written to `commission.deadline_status`.
    /// Persisted — renaming a token is a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Delayed => "delayed",
            Self::Late => "late",
        }
    }
}

impl std::fmt::Display for DeadlineStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Delayed => write!(f, "delayed"),
            Self::Late => write!(f, "late"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum DeadlineStatusError {
    ParseError,
    InvalidValue,
}

impl TryFrom<&str> for DeadlineStatus {
    type Error = DeadlineStatusError;

    /// Resolve a stored token back to its value.
    fn try_from(token: &str) -> Result<Self, Self::Error> {
        Ok(match token {
            "delayed" => Self::Delayed,
            "late" => Self::Late,
            _ => return Err(DeadlineStatusError::InvalidValue),
        })
    }
}

impl std::fmt::Display for DeadlineStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidValue => write!(f, "Invalid value"),
            Self::ParseError => write!(f, "Parsing error"),
        }
    }
}

impl FromStr for DeadlineStatus {
    type Err = DeadlineStatusError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "delayed" => Ok(Self::Delayed),
            "late" => Ok(Self::Late),
            _ => Err(DeadlineStatusError::InvalidValue),
        }
    }
}

/// The effective deadline-axis status at `now`. `Late` is derived, never
/// persisted: a passed deadline on a non-terminal commission *is* `Late`, and it
/// supersedes a standing `Delayed` without overwriting storage. Otherwise the
/// stored manual flag. Both adapters call this while rebuilding a [`Commission`].
pub fn derive_deadline_status(
    deadline: Option<DateTimeUtc>,
    lifecycle_step: &LifecycleStep,
    stored: Option<DeadlineStatus>,
    now: DateTimeUtc,
) -> Option<DeadlineStatus> {
    // No deadline means no deadline-axis status, even if a stored `Delayed`
    // lingers — it stays dormant until a deadline is set again.
    let deadline = deadline?;
    if deadline < now && !lifecycle_step.is_terminal() {
        Some(DeadlineStatus::Late)
    } else {
        stored
    }
}

/// One commission the deadline sweep must log as Late: deadline passed,
/// lifecycle not terminal, not yet logged (the sweep dedupes on the changelog).
/// Carries what the `late` entry needs to render without joins. The sweep only
/// logs; the state itself is derived by [`derive_deadline_status`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LapsedDeadline {
    /// The commission to log Late.
    pub id: CommissionId,
    /// The deadline that was missed (named in the Late entry's payload).
    pub deadline: DateTimeUtc,
    /// The standing manual flag at scan time — what the Late entry supersedes.
    pub status: Option<DeadlineStatus>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    // The deadline-status tokens round-trip and never collide.
    #[test]
    fn deadline_status_tokens_round_trip_and_never_collide() {
        let mut seen = BTreeSet::new();
        for status in DeadlineStatus::ALL {
            let token = status.as_str();
            assert!(seen.insert(token), "duplicate token {token:?}");
            assert_eq!(
                DeadlineStatus::try_from(token),
                Ok(*status),
                "token {token:?} must round-trip back to its value",
            );
        }
        assert_eq!(DeadlineStatus::ALL.len(), 2, "exactly the two values");
    }

    // A token outside the vocabulary is refused; the axes never bleed.
    #[test]
    fn unknown_deadline_status_tokens_do_not_parse() {
        assert_eq!(
            DeadlineStatus::try_from("waiting_for_input"),
            Err(DeadlineStatusError::InvalidValue),
            "direction axis ≠ deadline axis"
        );
        assert_eq!(
            DeadlineStatus::try_from(""),
            Err(DeadlineStatusError::InvalidValue)
        );
        assert_eq!(
            DeadlineStatus::try_from("Late"),
            Err(DeadlineStatusError::InvalidValue)
        );
    }

    // A fresh commission carries no deadline status, even born with a deadline.
    #[test]
    fn a_fresh_commission_has_no_deadline_status() {
        let c = Commission::create(
            "Ref".parse::<CommissionTitle>().unwrap(),
            crate::elements::user::UserId::new(crate::elements::did::Did::new(format!(
                "did:plc:{}",
                uuid::Uuid::now_v7()
            ))),
            chrono::Utc::now(),
            Some(chrono::Utc::now()),
        );
        assert_eq!(c.deadline_status, None);
    }

    // The lifecycle tokens round-trip and the terminal set is exactly
    // {completed, cancelled} — Disputed is not terminal.
    #[test]
    fn lifecycle_tokens_round_trip_and_terminal_is_exactly_closed_work() {
        let mut seen = BTreeSet::new();
        for step in LifecycleStep::ALL {
            let token = step.as_str();
            assert!(seen.insert(token), "duplicate token {token:?}");
            assert_eq!(
                LifecycleStep::try_from(token).map(|s| s.as_str()),
                Ok(token),
                "token {token:?} must round-trip back to its step",
            );
        }
        assert_eq!(LifecycleStep::ALL.len(), 6, "exactly the six states");

        let terminal: Vec<&str> = LifecycleStep::ALL
            .iter()
            .filter(|s| s.is_terminal())
            .map(|s| s.as_str())
            .collect();
        assert_eq!(terminal, vec!["completed", "cancelled"]);
    }

    // The direction-status tokens round-trip and never collide.
    #[test]
    fn direction_status_tokens_round_trip_and_never_collide() {
        let mut seen = BTreeSet::new();
        for status in DirectionStatus::ALL {
            let token = status.as_str();
            assert!(seen.insert(token), "duplicate token {token:?}");
            assert_eq!(
                DirectionStatus::try_from(token),
                Ok(*status),
                "token {token:?} must round-trip back to its value",
            );
        }
        assert_eq!(DirectionStatus::ALL.len(), 3, "exactly the three values");
    }

    // A token outside the vocabulary is refused, not guessed at.
    #[test]
    fn unknown_direction_status_tokens_do_not_parse() {
        assert_eq!(
            DirectionStatus::try_from("late"),
            Err(UnknownDirectionStatus),
            "deadline axis ≠ direction axis"
        );
        assert_eq!(DirectionStatus::try_from(""), Err(UnknownDirectionStatus));
        assert_eq!(
            DirectionStatus::try_from("Waiting for Input"),
            Err(UnknownDirectionStatus)
        );
    }

    // A fresh commission carries no direction status (the cleared state).
    #[test]
    fn a_fresh_commission_has_no_direction_status() {
        let c = Commission::create(
            "Ref".parse::<CommissionTitle>().unwrap(),
            crate::elements::user::UserId::new(crate::elements::did::Did::new(format!(
                "did:plc:{}",
                uuid::Uuid::now_v7()
            ))),
            chrono::Utc::now(),
            None,
        );
        assert_eq!(c.direction_status, None);
    }
}

/// Who may see a commission — the outermost gate, applied before the
/// composition's own [`effective_visibility`]. A fresh commission is
/// [`Private`](Visibility::Private); widening is an explicit later act.
#[derive(Debug, Clone, PartialEq)]
pub enum Visibility {
    /// Nobody outside the participants sees it at all, not even its existence.
    Private,
    /// Outsiders see only a status-only card.
    Listed,
    /// Outsiders see whatever sits under Description-visible surfaces.
    Public,
}

impl Visibility {
    /// The stable, lowercase token written to `commission.visibility`.
    /// Persisted — renaming a token is a migration.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Listed => "listed",
            Self::Public => "public",
        }
    }
}

/// Why a token failed to resolve to a [`Visibility`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownVisibility;

impl std::fmt::Display for UnknownVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("token is not one of: private, listed, public")
    }
}

impl std::error::Error for UnknownVisibility {}

impl TryFrom<&str> for Visibility {
    type Error = UnknownVisibility;

    /// Resolve a stored token back to its value.
    fn try_from(token: &str) -> Result<Self, Self::Error> {
        Ok(match token {
            "private" => Self::Private,
            "listed" => Self::Listed,
            "public" => Self::Public,
            _ => return Err(UnknownVisibility),
        })
    }
}
