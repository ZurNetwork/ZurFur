//! Commission ports: the canonical [`CommissionStore`] read surface and the
//! [`CommissionWrites`] write view. Commissions are entirely Index-side —
//! nothing on these surfaces ever touches atproto.

use async_trait::async_trait;

use crate::elements::{
    commission::{
        AsOwner, AsParticipant, ChannelPointer, Commission, CommissionBirth, CommissionComposition,
        CommissionFile, CommissionId, CommissionMarkup, CommissionStanding, CommissionSummary,
        DeadlineChange, DelayedChange, DirectionChange, Dispute, DisputeClear, ElementId, FileKey,
        FileSummary, GrantLevel, LateKinds, LifecycleMove, LifecycleStep, LockedNode,
        LockedStanding, MaturitySet, NewCommission, NewElement, NewSeat, NewSlot, Seat,
        SeatInvitation, SeatInvitationId, Slot, TabId, TabRow,
    },
    workflow::{Column, ColumnId, WorkflowId},
};
use identity::elements::user::UserId;
use kernel::{
    datetime::DateTimeUtc,
    ports::{Creator, New, WriteCapability},
};

pub use super::errors::{ElementNotFound, UnknownSurface, UnknownTab};

/// The **read** surface of Zurfur's record of commissions — pool-backed and
/// non-transactional. The one canonical commission read port: later work extends
/// it rather than growing siblings. Authorization is always the caller's, settled
/// before any of these is reached.
#[async_trait]
pub trait CommissionStore: Send + Sync {
    /// The commission `id` names, or `None`. An archived commission is still
    /// found — filtering it out is a listing projection's job.
    async fn find(&self, id: &CommissionId) -> anyhow::Result<Option<Commission>>;

    /// The [`CommissionSummary`] of the commission `id` names, or `None`: its node's name and
    /// level and whether it is archived, and nothing else of the row. A commission with no
    /// node reads as absent; an archived one is still found.
    async fn find_summary(&self, id: &CommissionId) -> anyhow::Result<Option<CommissionSummary>>;

    /// The [`CommissionStanding`] `user` holds on `commission`, or `None` when no
    /// commission has that id. An absent id and a stranger take the same one-row
    /// read, and the read carries no name and no whole row. The level is the
    /// commission's node's, Private when it has none. An archived commission is still found.
    async fn standing(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<Option<CommissionStanding>>;

    async fn current_column_of_workflow(
        &self,
        commission: &CommissionId,
        workflow_id: &WorkflowId,
    ) -> anyhow::Result<Option<Column>>;

    async fn current_position_in_column(
        &self,
        commission: &CommissionId,
        column_id: &ColumnId,
    ) -> anyhow::Result<Option<u8>>;

    /// The [`GrantLevel`] `user_id` holds on `commission`, or `None` if they hold
    /// no key. A revoked key hard-deletes, so this answers `None` immediately
    /// after revocation. A key only lifts the view; it never makes anyone a
    /// Participant.
    async fn view_grant(
        &self,
        commission_id: &CommissionId,
        user_id: &UserId,
    ) -> anyhow::Result<Option<GrantLevel>>;

    /// Whether `user` is a Participant of `commission` — the one authorization
    /// predicate every "a Participant does X" endpoint consumes. Reads the
    /// persisted `commission_participant` record, whose owner row is a permanent,
    /// irremovable floor. An unknown commission answers `false`, so callers
    /// collapse absent and hidden into one uniform 404.
    async fn is_participant(
        &self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<bool>;

    /// The commission's declared [`Seat`]s, keyed by their carrying elements' ids,
    /// in declaration order; empty for an unknown commission. Raw and Total-tier
    /// (they include `occupant`) — the viewer projection joins them against the
    /// projected composition to render vacant seats as published asks.
    async fn seats(&self, commission: &CommissionId) -> anyhow::Result<Vec<Seat>>;

    /// The commission's whole composition — every tab, surface mode and element —
    /// or `None` if absent. All three travel together because effective visibility
    /// is `min(tab, surface, element)`. The result is raw and server-internal:
    /// callers serialize only through the viewer projection.
    async fn load_composition(
        &self,
        id: &CommissionId,
    ) -> anyhow::Result<Option<CommissionComposition>>;

    /// The [`CommissionFile`] entry `key` names within `commission`, or `None`.
    /// Scoped in the query, so a key belonging to another commission answers
    /// `None` and the path never becomes a cross-commission existence oracle. The
    /// bytes live behind [`FileStore`](crate::ports::FileStore).
    async fn find_file(
        &self,
        commission: &CommissionId,
        key: FileKey,
    ) -> anyhow::Result<Option<CommissionFile>>;

    /// Every [`CommissionMarkup`] drawn on `file` within `commission`, in draw
    /// order. Empty — never an error — for a file with no annotations, and for one
    /// belonging to another commission (the same non-oracle scoping as
    /// [`find_file`](Self::find_file)).
    async fn markups_for_file(
        &self,
        commission: &CommissionId,
        file: FileKey,
    ) -> anyhow::Result<Vec<CommissionMarkup>>;

    /// The pending [`SeatInvitation`] for `(commission, seat, user)`, or `None`.
    /// Only ever a pending offer; underpins the idempotent re-invite. Scoped in
    /// the query, so a seat id from another commission never matches.
    async fn find_pending_seat_invitation(
        &self,
        commission: &CommissionId,
        seat: &ElementId,
        user: &UserId,
    ) -> anyhow::Result<Option<SeatInvitation>>;

    /// The commissions `owner` owns — owner-POV only, not the participant
    /// projection. Archived commissions are excluded; ordered by
    /// [`CommissionId`], unpaginated.
    async fn list_owned_by(&self, owner: &UserId) -> anyhow::Result<Vec<Commission>>;

    /// The commissions `user` is a Participant of, read from the same
    /// participant record as [`is_participant`](Self::is_participant), each named and
    /// levelled by its node; one with no node is left out. Archived commissions are
    /// included; hiding them is the caller's. Ordered by [`CommissionId`]; empty for a
    /// user who takes part in none.
    async fn list_participating(&self, user: &UserId) -> anyhow::Result<Vec<CommissionSummary>>;

    /// Every file entry of `commission` as a [`FileSummary`] (key and upload
    /// time, never the uploader), ordered by [`FileKey`] (upload order). Empty
    /// for a commission with no files, and for an unknown one. The names live
    /// behind [`FileStore::filename`](crate::ports::FileStore::filename).
    async fn files(&self, commission: &CommissionId) -> anyhow::Result<Vec<FileSummary>>;

    /// Every Slot declared on `commission`, in declaration order. Empty for a
    /// commission with no Slots, and for an unknown one.
    async fn slots(&self, commission: &CommissionId) -> anyhow::Result<Vec<Slot>>;

    /// The ids of the commissions the deadline sweep may owe a `late` entry as of `now`:
    /// deadline strictly before `now`, step not in `not_lapsing`, and no entry of `late_kinds`'
    /// mark recorded since the last of its re-arming kinds. Ordered by deadline, then id. Only a
    /// scan: each one is re-read under its own lock before anything is written.
    async fn lapsed_deadlines(
        &self,
        now: DateTimeUtc,
        not_lapsing: &[LifecycleStep],
        late_kinds: &LateKinds,
    ) -> anyhow::Result<Vec<CommissionId>>;
}

/// The **write** surface of Zurfur's record of commissions — reachable only on an
/// open `UnitOfWork` (`uow.commissions()`). A create's authority is the den's write
/// gate's create [`WriteCapability`], which [`create`](Self::create) takes; a lifecycle move's,
/// a dispute's raise and clear, a direction status's, a deadline's and a Delayed flag's is a
/// Participant's, and a rating's the Owner's, which their writes take. Every other method's
/// authority, and the commission's own existence, are the caller's checks, settled before it
/// is reached.
#[async_trait]
pub trait CommissionWrites: Send {
    /// Persist a new commission under the id `capability` was issued for, at `birth`'s
    /// node, owner and time, with what `commission` chose, in the starting state
    /// [`CommissionStart::default`](crate::elements::commission::CommissionStart::default)
    /// names. Its skeleton tab rows and its owner's participant row are written with it, so a
    /// tabless or owner-less one can't exist.
    async fn create(
        &mut self,
        capability: &WriteCapability<New<CommissionId>, Creator>,
        birth: &CommissionBirth,
        commission: &NewCommission,
    ) -> anyhow::Result<()>;

    /// Move the commission `capability` was issued for from `change`'s held step to its new
    /// one, by compare-and-set: the row changes only while it still holds the held step.
    /// Answers `true` only when exactly one row changed.
    async fn set_lifecycle(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &LifecycleMove,
    ) -> anyhow::Result<bool>;

    /// Put the commission `capability` was issued for into Disputed from `dispute`'s held step,
    /// storing that step and when the dispute began, by compare-and-set: the row changes only
    /// while it still holds the held step. Answers `true` only when exactly one row changed.
    async fn set_dispute(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        dispute: &Dispute,
    ) -> anyhow::Result<bool>;

    /// Return the commission `capability` was issued for from Disputed to the step its dispute
    /// left, which must be `clear`'s, and forget the dispute, by compare-and-set: the row
    /// changes only while it is still Disputed from that step. Answers `true` only when exactly
    /// one row changed.
    async fn clear_dispute(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        clear: &DisputeClear,
    ) -> anyhow::Result<bool>;

    /// Declare a [`NewSeat`] into a declared surface, as one element plus one
    /// satellite row sharing the seat's id. Refuses an absent/foreign tab with
    /// [`UnknownTab`] and an undeclared surface with [`UnknownSurface`]. The
    /// caller appends the matching `seat_declared` entry in this same unit.
    async fn declare_seat(&mut self, seat: &NewSeat) -> anyhow::Result<()>;

    /// Persist a freshly issued, pending [`SeatInvitation`]. At most one pending
    /// invitation per (seat, invited user) — a duplicate is a no-op rather than a
    /// second row; different Users may each hold one to the same seat.
    async fn create_seat_invitation(
        &mut self,
        invitation: &SeatInvitation,
    ) -> anyhow::Result<SeatInvitation>;

    /// Transition a pending seat invitation to revoked. Idempotent on a
    /// non-pending or absent invitation — a no-op, not an error.
    async fn revoke_seat_invitation(&mut self, id: &SeatInvitationId) -> anyhow::Result<()>;

    /// Contribute a [`NewElement`] into a declared surface. Append order is
    /// assigned here within `(tab, surface, band)`, and implementations must
    /// serialize concurrent appends into one such group so they neither race to a
    /// position nor abort on its uniqueness at commit. The element is born
    /// `Total`, and the payload round-trips as an equal JSON value. Refuses with
    /// [`UnknownTab`] or [`UnknownSurface`].
    async fn add_element(&mut self, element: &NewElement) -> anyhow::Result<()>;

    /// Remove `element` from the commission's composition — one row, plus the
    /// satellites and pending invitations that cascade off its identity — and
    /// renumber the remaining `(tab, surface, band)` group in the same
    /// transaction. Refuses with [`ElementNotFound`].
    async fn remove_element(
        &mut self,
        commission: &CommissionId,
        element: &ElementId,
    ) -> anyhow::Result<()>;

    /// Record a file entry's [`CommissionFile`] link, tying an uploaded file's
    /// [`FileKey`] to its commission, alongside the `file_added` changelog entry
    /// the caller appends in this same unit. The bytes are stored before this unit
    /// through [`FileStore::put`](crate::ports::FileStore::put), never inside it.
    /// The row is bookkeeping, not a [`Fact`](crate::elements::commission::Fact).
    async fn add_file(&mut self, file: &CommissionFile) -> anyhow::Result<()>;

    /// Record a [`CommissionMarkup`] — one annotation's geometry — alongside the
    /// `markup_added` changelog entry the caller appends in this same unit.
    /// Append-only by omission: there is deliberately no update or delete. The row
    /// is bookkeeping, not a [`Fact`](crate::elements::commission::Fact).
    async fn add_markup(&mut self, markup: &CommissionMarkup) -> anyhow::Result<()>;

    /// Declare Slots on the commission, as a batch in one write: each [`NewSlot`]
    /// becomes an ordinary element in the named surface plus a satellite row keyed
    /// by that element's id. All-or-nothing — the first refusal aborts the write.
    /// Same gates as [`add_element`](Self::add_element). Nothing here fills a
    /// Slot; no occupant is representable.
    async fn declare_slots(&mut self, slots: &[NewSlot]) -> anyhow::Result<()>;

    /// Whether the commission bears any [`Fact`](crate::elements::commission::Fact)
    /// — the single predicate deciding hard-delete legality. A read on the write
    /// view deliberately: the gate runs in the same transaction as the delete it
    /// guards, so there is no TOCTOU window. An unknown commission answers `false`.
    /// Every new fact kind's storage must join this predicate.
    async fn commission_has_facts(&mut self, id: &CommissionId) -> anyhow::Result<bool>;

    /// Hard-delete the commission, taking every child row with it by cascade. The
    /// caller gates this on
    /// [`commission_has_facts`](Self::commission_has_facts) in the same unit, so
    /// the cascade can never take a fact. Deleting an absent commission is a
    /// no-op.
    async fn delete(&mut self, id: &CommissionId) -> anyhow::Result<()>;

    /// Archive (`Some(when)`) or un-archive (`None`) the commission; the record
    /// and its facts survive, only active-view listings lose it. Returns whether
    /// the state actually transitioned — a repeat keeps the original stamp and
    /// answers `false` — so the caller keys its changelog append on a real change
    /// in the same unit.
    async fn set_archived(
        &mut self,
        id: &CommissionId,
        archived_at: Option<DateTimeUtc>,
    ) -> anyhow::Result<bool>;

    /// Rate the commission `capability` was issued for, from `change`'s held rating (none while
    /// unrated) to its new one, by compare-and-set: the row changes only while it still holds
    /// the held rating and Graphic flag. Replace-only: the change always carries a rating.
    /// Answers `true` only when exactly one row changed.
    async fn set_maturity(
        &mut self,
        capability: &WriteCapability<CommissionId, AsOwner>,
        change: &MaturitySet,
    ) -> anyhow::Result<bool>;

    /// Set (`Some`) or clear (`None`) the commission's external linked-channel
    /// pointer. Returns whether the stored value actually changed, so the caller
    /// keys its `channel_linked`/`channel_unlinked` append on a real change in
    /// this same unit.
    async fn set_linked_channel(
        &mut self,
        id: &CommissionId,
        channel: Option<&ChannelPointer>,
    ) -> anyhow::Result<bool>;

    /// Issue `to_user` a key to see `commission` at `level`. At most one key per
    /// (commission, user): re-granting replaces the level. The row is a pure key —
    /// who issued it and when live only in the changelog entry the caller appends
    /// in this same unit.
    async fn grant_view(
        &mut self,
        commission: &CommissionId,
        to_user: &UserId,
        level: GrantLevel,
    ) -> anyhow::Result<()>;

    /// Revoke `to_user`'s view grant by hard-deleting the key row; because
    /// visibility is enforced at serialization, it takes effect on the next
    /// render. Returns whether a key was actually removed, so the caller keys its
    /// changelog append on a real transition.
    async fn revoke_view(
        &mut self,
        commission: &CommissionId,
        to_user: &UserId,
    ) -> anyhow::Result<bool>;

    /// Set or clear the direction status of the commission `capability` was issued for, from
    /// `change`'s held status to its new one, by compare-and-set: the row changes only while it
    /// still holds the held status. Answers `true` only when exactly one row changed.
    async fn set_direction_status(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &DirectionChange,
    ) -> anyhow::Result<bool>;

    /// Set or clear the deadline of the commission `capability` was issued for, from
    /// `change`'s held deadline to its new one, by compare-and-set: the row changes only while
    /// it still holds the held deadline. Answers `true` only when exactly one row changed.
    async fn set_deadline(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &DeadlineChange,
    ) -> anyhow::Result<bool>;

    /// Raise or take down the Delayed flag of the commission `capability` was issued for, from
    /// `change`'s held flag to its new one, by compare-and-set: the row changes only while it
    /// still holds the held flag. Answers `true` only when exactly one row changed.
    async fn set_deadline_status(
        &mut self,
        capability: &WriteCapability<CommissionId, AsParticipant>,
        change: &DelayedChange,
    ) -> anyhow::Result<bool>;
}

/// The **read** side of a commission unit of work — the same lookups as
/// [`CommissionStore`], on the unit's own connection so they see its uncommitted
/// writes and can hold row locks until commit.
#[async_trait]
pub trait CommissionReads: Send {
    /// The commission `id` names, or `None`; as [`CommissionStore::find`].
    async fn find(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>>;

    /// [`find`](Self::find), with the row locked (`FOR NO KEY UPDATE`) for the
    /// rest of the unit; child-row inserts by others are not blocked.
    async fn find_for_update(&mut self, id: &CommissionId) -> anyhow::Result<Option<Commission>>;

    /// [`CommissionStore::standing`], with the commission row locked
    /// (`FOR NO KEY UPDATE`) for the rest of the unit, beside the node the commission
    /// points at; `None` when it is absent or has no node.
    /// Every change to someone's standing (evict, leave, role change, Admin grant
    /// or revoke, transfer) locks the commission row first, so this answer holds
    /// until the unit ends.
    async fn standing_for_update(
        &mut self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<Option<LockedStanding>>;

    /// The node `commission` points at and that node's level, with the commission row locked
    /// (`FOR NO KEY UPDATE`) for the rest of the unit; `None` when it is absent or has no node.
    /// Reads no viewer: it is the system's own act's lock, which no user's standing grants.
    async fn lock_for_system(
        &mut self,
        commission: &CommissionId,
    ) -> anyhow::Result<Option<LockedNode>>;

    /// Whether `commission` is still owed a `late` entry: no entry of `late_kinds`' mark
    /// recorded since the last of its re-arming kinds. [`CommissionStore::lapsed_deadlines`]'s
    /// anti-join, for one commission.
    async fn late_owed(
        &mut self,
        commission: &CommissionId,
        late_kinds: &LateKinds,
    ) -> anyhow::Result<bool>;

    /// Whether `user` is a Participant of `commission`; as
    /// [`CommissionStore::is_participant`].
    async fn is_participant(
        &mut self,
        commission: &CommissionId,
        user: &UserId,
    ) -> anyhow::Result<bool>;

    /// The tab `tab` names within `commission`, locked for the rest of the unit,
    /// or `None` — an absent id and a foreign one are indistinguishable. The one
    /// serialization point of every composition write; the skeleton check belongs
    /// to the caller.
    async fn tab_for_update(
        &mut self,
        commission: &CommissionId,
        tab: &TabId,
    ) -> anyhow::Result<Option<TabRow>>;
}

/// A commission unit of work: [`CommissionReads`] + [`CommissionWrites`] on one
/// connection. Vended by `UnitOfWork::commissions`.
pub trait CommissionRepo: CommissionReads + CommissionWrites {}

impl<T: CommissionReads + CommissionWrites> CommissionRepo for T {}
