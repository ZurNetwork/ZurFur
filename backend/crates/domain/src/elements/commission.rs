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
    /// The commission's Title — fixed and always present; every other content
    /// facet is later composition.
    pub title: String,
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
    /// When the commission was created.
    pub created_at: DateTimeUtc,
}

impl Commission {
    /// Create a commission owned by `owner`, born in [`LifecycleStep::Draft`].
    ///
    /// Mints the id (`CommissionId::new(Uuid::now_v7())`), records the caller-supplied
    /// `title` and optional `deadline`, and stamps `created_at` from `now`. Authority
    /// is the caller's concern (a signed-in User; no Account needed — ZMVP-47), settled
    /// before this is reached; this constructor only shapes the row.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{commission::{Commission, LifecycleStep}, user::UserId};
    ///
    /// let owner = UserId::new(uuid::Uuid::now_v7());
    /// let c = Commission::create("A ref sheet".to_string(), owner, Utc::now(), None);
    /// assert_eq!(c.owner_id, owner);                             // the creator owns it
    /// assert!(matches!(c.lifecycle_step, LifecycleStep::Draft)); // born in Draft
    /// assert_eq!(c.title, "A ref sheet");
    /// ```
    pub fn create(
        title: String,
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
}
