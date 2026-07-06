//! The [`Commission`] — the platform's most basic unit of work and the aggregator
//! of the work done under it (DESIGN/Commission).
//!
//! This is the **birth** shape (ZMVP-65): only the fixed metadata that always
//! exists — a UUIDv7 [`CommissionId`], a `Title`, the owning [`UserId`], a single
//! [`LifecycleStep`], a nullable deadline, and a creation stamp. A commission is
//! created by any authenticated User with **no Account required** (a user-scoped
//! write; ZMVP-47, DD 26247170). Everything else the glossary describes — the
//! content tree of Surfaces/Components, Seats/Slots, participants beyond the
//! creator, the managing-account association, and lifecycle/status transitions —
//! materializes in later tickets, not here.
//!
//! A commission is **isolated from accounts**: it survives account deletion and its
//! participants are always Users, never accounts. Visibility is carried as a flat
//! [`Visibility`] field defaulting to `Private` (the closed-door policy) — the three
//! values survive as the aliases the per-surface Surfaces DD (`28246028`) keeps for
//! the future root-surface mode (`Private` = root at `Total`), so when the content
//! tree lands the field is reinterpreted, not replaced.
//!
//! The [`fact`] submodule carries the [`Fact`] contract (ZMVP-67) — what it means
//! for a type to be commission-anchored evidence that blocks hard deletion. The
//! [`changelog`] submodule carries the commission's append-only memory (ZMVP-87):
//! the frozen [`ChangelogEntryKind`] taxonomy, the entry shapes, and the
//! [`ChannelPointer`] "where we talk" value.

pub mod changelog;
pub mod fact;

pub use changelog::{
    ChangelogEntry, ChangelogEntryKind, ChannelPointer, ChannelPointerError, NewChangelogEntry,
};
pub use fact::Fact;

use std::ops::Deref;

use crate::{datetime::DateTimeUtc, elements::user::UserId};

/// The app-private, stable handle for a [`Commission`].
///
/// A UUIDv7 wrapped for type safety, mirroring [`crate::elements::account::AccountId`]
/// and [`crate::elements::user::UserId`]. The UUIDv7 carries the creation timestamp;
/// Deref exposes the inner UUID for foreign keys and lookups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommissionId(uuid::Uuid);

impl CommissionId {
    /// Wraps an already-minted UUIDv7. Mirrors [`crate::elements::account::AccountId::new`]:
    /// the app mints the key (PG16 has no native `uuidv7()`), the domain only names it.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl Deref for CommissionId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A commission's Title, validated on the way in.
///
/// Surrounding whitespace is trimmed; the result must be non-empty. The Title is
/// the one always-present content facet of a commission (DESIGN/Commission), so a
/// blank one is rejected rather than stored — the same construction-time gate
/// [`crate::elements::account::AccountName`] applies to account names (no length cap
/// is imposed here yet).
///
/// ```
/// use domain::elements::commission::CommissionTitle;
///
/// let title = CommissionTitle::try_new("  A ref sheet  ").unwrap();
/// assert_eq!(title.as_str(), "A ref sheet"); // trimmed
///
/// assert!(CommissionTitle::try_new("   ").is_err()); // empty after trim
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommissionTitle(String);

/// Why a string was rejected as a commission title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommissionTitleError {
    /// Empty once trimmed. Example: `""` or `"   "`.
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
    /// Validate and wrap a title: trim surrounding whitespace, then reject an empty
    /// result with [`CommissionTitleError::Empty`].
    pub fn try_new(raw: impl Into<String>) -> Result<Self, CommissionTitleError> {
        let trimmed = raw.into().trim().to_owned();
        if trimmed.is_empty() {
            return Err(CommissionTitleError::Empty);
        }
        Ok(Self(trimmed))
    }

    /// The validated, trimmed title as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A created commission and its fixed metadata (ZMVP-65).
///
/// Build one with [`Commission::create`], which stamps a fresh UUIDv7 id and opens
/// it in [`LifecycleStep::Draft`] owned by its creator. The struct holds no
/// participant list, content tree, or managing account — those are later tickets;
/// this is only the always-present envelope. Persisting it is one private-side
/// write ([`crate::ports::CommissionWrites::create`]).
///
/// References: [`Commission::create`], [`crate::ports::CommissionWrites`],
/// DESIGN/Commission (`3276807`), Ask-for-Art (`28114957`) D0.
#[derive(Debug)]
pub struct Commission {
    /// The app-private id (UUIDv7, so it sorts by creation time).
    pub id: CommissionId,
    /// The commission's Title — fixed and always present, validated non-empty; every
    /// other content facet is later composition. See [`CommissionTitle`].
    pub title: CommissionTitle,
    /// The User who created the commission and owns it. The owner is permanent in
    /// the domain model (transfer is an explicit later act; DESIGN/Commission);
    /// birth just records the creator here.
    pub owner_id: UserId,
    /// The single lifecycle state the commission is in; a fresh one is
    /// [`LifecycleStep::Draft`].
    pub lifecycle_step: LifecycleStep,
    /// Who may see the commission; a fresh one is [`Visibility::Private`] (the
    /// closed-door default — AC3).
    pub visibility: Visibility,
    /// The nullable-but-fixed deadline envelope field — `None` when the commission
    /// carries no deadline (DESIGN/Commission).
    pub deadline: Option<DateTimeUtc>,
    /// The external **linked channel** pointer — "where we talk" (ZMVP-87,
    /// Changelog DD Decision 2) — or `None` while no channel is declared. Owner-set,
    /// changelog-recorded on set/clear, rendered as an opaque pointer.
    pub linked_channel: Option<ChannelPointer>,
    /// When the commission was **archived** — `None` while active (ZMVP-68).
    ///
    /// Archive is the soft path (Deletion DD `3014657`): the mandatory route once
    /// facts exist, and available regardless of facts (hard delete, ZMVP-66, is
    /// the fact-gated path). An archived commission is meant to disappear from
    /// **active views** — listing projections are responsible for filtering on
    /// this field (active-view filtering lands with the S1 listing work) — but
    /// the record and its facts survive intact and stay queryable by its
    /// Participants.
    /// Owner-only in both directions, and both directions are changelog entries
    /// ([`ChangelogEntryKind::Archived`]/[`ChangelogEntryKind::Unarchived`]);
    /// un-archive is an explicit owner act that returns the commission to active
    /// views (Engineer ruling 2026-07-05, recorded on ZMVP-68). Archive stays in
    /// the owner-only reserve even when Commission Admin lands (Structural
    /// Authority DD `29425666` Decision 2).
    pub archived_at: Option<DateTimeUtc>,
    /// When the commission was created.
    pub created_at: DateTimeUtc,
}

impl Commission {
    /// Create a commission owned by `owner`, born in [`LifecycleStep::Draft`].
    ///
    /// Mints the id (`CommissionId::new(Uuid::now_v7())`), records the already-validated
    /// [`CommissionTitle`] and optional `deadline`, and stamps `created_at` from `now`.
    /// The title is validated at the boundary ([`CommissionTitle::try_new`]) before this
    /// is reached, so this constructor is infallible — mirroring how [`Account::open`]
    /// takes an already-validated [`AccountName`]. Authority (a signed-in User; no
    /// Account needed — ZMVP-47) is the caller's concern, settled before this is reached.
    ///
    /// [`Account::open`]: crate::elements::account::Account::open
    /// [`AccountName`]: crate::elements::account::AccountName
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{commission::{Commission, CommissionTitle, LifecycleStep}, user::UserId};
    ///
    /// let owner = UserId::new(uuid::Uuid::now_v7());
    /// let title = CommissionTitle::try_new("A ref sheet").unwrap();
    /// let c = Commission::create(title, owner, Utc::now(), None);
    /// assert_eq!(c.owner_id, owner);                             // the creator owns it
    /// assert!(matches!(c.lifecycle_step, LifecycleStep::Draft)); // born in Draft
    /// assert_eq!(c.title.as_str(), "A ref sheet");
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
            linked_channel: None,
            archived_at: None,
        }
    }
}

/// The single lifecycle state a commission holds (DESIGN/Commission).
///
/// A commission is always in exactly one of these, and the state is moved
/// **explicitly by a participant**, never by a system event. Only the birth state
/// ([`Draft`](LifecycleStep::Draft)) is exercised in ZMVP-65; the transitions between
/// states are later tickets.
#[derive(Debug, Clone)]
pub enum LifecycleStep {
    /// Just created. No content commitments and no facts. Hard delete is possible.
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
    /// The stable, lowercase wire/storage token for this state — the value the pg
    /// adapter writes to the `commission.lifecycle` column. Stable across releases
    /// (it is persisted), so renaming a token is a migration, not a free edit.
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

    /// Resolve a stored token back to its step, or `None` for a token outside the
    /// vocabulary — on a read path that means row tampering or a missed migration
    /// and surfaces as an error, never a silent default (ZMVP-87 read port).
    pub fn parse(token: &str) -> Option<Self> {
        Some(match token {
            "draft" => Self::Draft,
            "batched" => Self::Batched,
            "active" => Self::Active,
            "completed" => Self::Completed,
            "cancelled" => Self::Cancelled,
            "disputed" => Self::Disputed,
            _ => return None,
        })
    }
}

/// Who may see a commission (DESIGN/Commission, the Closed-Door Policy).
///
/// The three values are the flat aliases the per-surface Surfaces DD (`28246028`)
/// preserves for the future root-surface mode — `Private` = root at `Total`,
/// `Listed` = root at `Presentation`, `Public` = root at `Description`. A birth
/// commission defaults to [`Private`](Visibility::Private); widening is an explicit
/// later act, and when the content tree lands this field is reinterpreted as the
/// root mode rather than replaced.
#[derive(Debug, Clone)]
pub enum Visibility {
    /// Closed door — nobody outside the participants sees the commission at all,
    /// not even its existence. The default at birth.
    Private,
    /// Outsiders see only a status-only card (title/alias, stage, position,
    /// maturity) — never the brief, client, price, or file entries.
    Listed,
    /// Outsiders see whatever the owner has composed under Description-visible
    /// surfaces; everything else stays dark.
    Public,
}

impl Visibility {
    /// The stable, lowercase wire/storage token for this value — what the pg adapter
    /// writes to the `commission.visibility` column. Stable across releases (it is
    /// persisted), so renaming a token is a migration, not a free edit.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::Listed => "listed",
            Self::Public => "public",
        }
    }

    /// Resolve a stored token back to its value, or `None` for one outside the
    /// vocabulary — the same tamper-surfacing contract as
    /// [`LifecycleStep::parse`].
    pub fn parse(token: &str) -> Option<Self> {
        Some(match token {
            "private" => Self::Private,
            "listed" => Self::Listed,
            "public" => Self::Public,
            _ => return None,
        })
    }
}
