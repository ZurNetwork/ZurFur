//! The [`Commission`] — the platform's first-class unit of work. **Stub: shapes
//! sketched, no behaviour yet.**
//!
//! A commission is the basic unit and, in domain terms, the *aggregator* of
//! work: it aggregates [`Participant`]s (assigned or merely invited), [`Slot`]s
//! holding characters and reference art, and a [`Lifecycle`] state machine
//! (DESIGN/Commission). Notably it does **not** belong to an account directly —
//! a participant is a [`ParticipantRef`] (a user or a golem), never an account,
//! though one account *manages* it via `current_managing_account_id` so a
//! commission can be transferred between accounts. The types below are the data
//! shapes only; constructors, transitions, and invariants are deferred until the
//! `workflow`/commission namespace is built out.

use std::collections::HashSet;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId, blob::BlobId, character::CharacterId, did::Did, golem::GolemId,
        markdown::Markdown, user::UserId,
    },
};

/// The identity of a [`Commission`]. Stub: a UUIDv7 wrapped for type safety.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CommissionId(uuid::Uuid);

/// A participant's function *within a commission* — distinct from an account
/// [`crate::elements::role::Role`].
///
/// A participant may hold more than one (hence stored in a [`HashSet`] on
/// [`Participant`]): e.g. a creator on one commission can also be a client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommissionRole {
    /// A creator in the commission
    Creator,
    /// A client in the commission
    Client,
}

/// A reference to whoever can take part in a commission.
///
/// Deliberately a user *or* a golem, never an account (DESIGN/Commission): the
/// actors at the table are principals, not accounts. See
/// [`crate::elements::golem`] for why a [`Golem`](GolemId) is a participant in
/// its own right.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParticipantRef {
    User(UserId),
    Golem(GolemId),
}

/// One actor's seat in a commission: who they are, what role(s) they play, and
/// how they got there. Stub shape.
#[derive(Debug, Clone)]
pub struct Participant {
    /// The ParticipantRef of the participant. As per design, not an account.
    pub subject: ParticipantRef,
    /// What roles of this participant is
    pub roles: HashSet<CommissionRole>,
    /// Whether this participant is actively assigned (vs merely invited) —
    /// "assigned is mostly for keeping track on who did what" (DESIGN/Commission).
    pub assigned: bool,
    /// Who this user was invited by. As per rules as written, Golems may add users!
    pub invited_by: Option<ParticipantRef>,
    /// The title given to this participant
    pub title: Option<String>,
}

/// A reference to a [`Blob`](BlobId) together with its owner's [`Did`].
///
/// The owner DID travels with the [`BlobId`] because blobs live on the owner's
/// PDS (content-addressed; see [`crate::elements::blob`]), so resolving the bytes
/// needs to know whose repository to fetch from.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlobRef {
    pub owner: Did,
    pub id: BlobId,
}

/// A single reference image pinned to a [`Slot`], with optional notes. Stub shape.
#[derive(Debug, Clone)]
pub struct SlotReference {
    pub blob: BlobRef,
    pub notes: Option<Markdown>,
}

/// One workspace within a commission: a numbered slot holding at most one
/// character plus its reference art. Stub shape.
///
/// Only one [`Character`](CharacterId) may occupy a slot at a time
/// (DESIGN/Character); a character must be *assigned* to a slot to be used in the
/// commission. The character may be present without its Keeper being a
/// participant (gift art, fan art, secret art).
#[derive(Debug, Clone)]
pub struct Slot {
    pub slot_number: u8,
    pub character_id: Option<CharacterId>,
    /// Multiple art pieces may be used as references
    pub references: Vec<SlotReference>,
}

/// A commission — the aggregator of work. **Stub: a data shape, no behaviour.**
///
/// Bundles the participants, slots, lifecycle, and visibility that make up a
/// piece of work (DESIGN/Commission). `current_managing_account_id` is the one
/// link to an account — the manager that can be reassigned to transfer the
/// commission — while `owner` and `participants` are [`ParticipantRef`]s, never
/// accounts. `deleted_at` is the soft-delete marker. Constructors and lifecycle
/// transitions are not modelled here yet.
#[derive(Debug)]
pub struct Commission {
    /// The ID of the commission
    pub id: CommissionId,
    pub title: String,
    pub description: Markdown,
    /// The current lifecycle state of the commission
    pub lifecycle: Lifecycle,
    /// Where the commission sits in the give-direction / request-changes loop,
    /// or `None` when no direction is pending. See [`DirectionStatus`].
    pub direction: Option<DirectionStatus>,
    /// Deadline health, or `None` when on track. See [`DeadlineStatus`].
    pub deadline_status: Option<DeadlineStatus>,
    /// The account that currently controls the commission. Allows for transferring commissions
    /// between accounts.
    pub current_managing_account_id: Option<AccountId>,
    /// The participant who owns the commission (a user or golem, not an account).
    pub owner: ParticipantRef,
    pub visibility: Visibility,
    /// Every participant and their role in the commission
    pub participants: Vec<Participant>,
    /// The agreed delivery deadline, if any. See [`deadline_status`](Commission::deadline_status).
    pub deadline: Option<DateTimeUtc>,
    pub slots: Vec<Slot>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
    /// Soft-delete marker: `Some(when)` once deleted, else `None`.
    pub deleted_at: Option<DateTimeUtc>,
}

/// The commission's lifecycle state. Mutually exclusive — a commission is in
/// exactly one. **Stub: variants only; no transition rules enforced yet.**
///
/// Per DESIGN/Commission this is the state machine every actor's authority is
/// bounded by (a participant can never violate it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Lifecycle {
    Draft,
    Batched,
    Active,
    Completed,
    Cancelled,
    Disputed,
}

/// Deadline health flags, set only when a commission is off the happy path.
/// Stub: variants only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeadlineStatus {
    Delayed,
    Late,
}

/// Where a commission sits in the direction/approval loop between client and
/// creator. Stub: variants only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DirectionStatus {
    WaitingForInput,
    ChangesRequested,
    WaitingForApproval,
}

/// How widely a commission is exposed. Stub: variants only.
///
/// `Private` (only participants), `Listed` (reachable but not surfaced), and
/// `Public` (openly listed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    Private,
    Listed,
    Public,
}
