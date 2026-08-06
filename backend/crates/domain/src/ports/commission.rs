//! Commission ports: the canonical [`CommissionStore`] read surface and the
//! [`CommissionWrites`] write view (ZMVP-65/67/87). Commissions are entirely
//! Index-side — nothing on these surfaces ever touches atproto.

use async_trait::async_trait;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId,
        commission::{
            ChannelPointer, Commission, CommissionComposition, CommissionFile, CommissionId,
            DeadlineStatus, DirectionStatus, ElementId, FileKey, GrantLevel, LapsedDeadline,
            NewElement, NewSeat, NewSlot, Placement, Seat, SeatInvitation, SeatInvitationId,
        },
        maturity::Maturity,
        user::UserId,
    },
};

/// The **read** surface of Zurfur's record of commissions — pool-backed and
/// non-transactional, shaped like [`AccountStore`](crate::ports::AccountStore)
/// (ZMVP-87). This is the **one canonical commission read port**: later tickets
/// extend it rather than growing siblings, so every read of "does it exist / who
/// is in it" shares one contract.
#[async_trait]
pub trait CommissionStore: Send + Sync {
    /// Resolve a [`CommissionId`] back to its [`Commission`], or `None` if no
    /// such commission exists.
    ///
    /// An **archived** commission is still found (ZMVP-68): archive removes it
    /// from *active views* — listing projections are responsible for filtering
    /// on [`Commission::archived_at`] (deferred to the S1 listing work) — never
    /// from its Participants' reach (the record and its facts survive and stay
    /// queryable, and the owner resolves it here to un-archive it).
    async fn find(&self, id: CommissionId) -> anyhow::Result<Option<Commission>>;

    /// The commission's **current placement** — the latest row of its append-only
    /// placement log (ZMVP-70; Ownership Separation DD `29130754`), read from the
    /// denormalized current-placement pointer the write side keeps in step. `None`
    /// when the commission has never been placed (a valid state — placement is
    /// optional). Positioning is account-side view state and confers no
    /// in-commission authority (Decision 8).
    async fn current_placement(
        &self,
        commission: CommissionId,
    ) -> anyhow::Result<Option<Placement>>;

    /// The commission's whole **placement log** in append order (ascending `seq`),
    /// so the current placement is the last row and the origin the first (ZMVP-70).
    /// The log is append-only and never rewritten; an unplaced commission has an
    /// empty log. Used to prove the current-placement pointer equals the latest row.
    async fn placement_log(&self, commission: CommissionId) -> anyhow::Result<Vec<Placement>>;

    /// The [`GrantLevel`] an `account` currently holds on `commission`, or `None`
    /// if it holds no key (ZMVP-70; Ownership Separation DD `29130754` Decision 3).
    /// This is the building block of the read-side "best key via membership" lift
    /// (the serializer, a later ticket); a revoked key hard-deletes, so this
    /// answers `None` immediately after revocation (Decision 5). A key is only a
    /// view — it never makes the account's members Participants (Decision 8), so
    /// [`is_participant`](Self::is_participant) is unaffected by any grant.
    async fn view_grant(
        &self,
        commission: CommissionId,
        account: AccountId,
    ) -> anyhow::Result<Option<GrantLevel>>;

    /// Whether `user` is a **Participant** of `commission` — the authorization
    /// predicate every "a Participant does X" endpoint consumes (the changelog
    /// read/note write here; lifecycle/status moves in ZMVP-84/85; …).
    ///
    /// **Persisted membership** (ZMVP-76, Engineer ruling): the predicate reads
    /// the explicit `commission_participant` membership record, not a computed
    /// owner-∪-seated union. The owner's row is inserted with the commission
    /// itself (and backfilled for commissions that predate the table) and is the
    /// **permanent floor** — the owner IS a Participant without holding any Seat
    /// (DESIGN/Commission — "a commission has at least one Participant: its
    /// owner, who is permanent"), and the row is irremovable: no write removes a
    /// participant, and the pg store additionally refuses deleting the owner's
    /// row at the database. Membership independent of both the `owner_id` column
    /// and seat occupancy is what lets ZMVP-69's prior owner *remain* a
    /// Participant and ZMVP-79 add seated members behind this same signature —
    /// do not grow a second predicate.
    ///
    /// An unknown commission has no participants, so it answers `false` — which
    /// is what lets a caller collapse "absent" and "hidden" into one uniform 404
    /// (the closed-door policy: existence is never leaked to outsiders).
    async fn is_participant(&self, commission: CommissionId, user: UserId) -> anyhow::Result<bool>;

    /// The commission's declared [`Seat`]s (ZMVP-76) — the interpreted satellite
    /// rows, keyed by their carrying elements' ids, in declaration order. An
    /// unknown commission (or one with no seats) is simply the empty list.
    ///
    /// **This is the ask-projection hook (AC4):** the viewer projection
    /// (ZMVP-170) joins these rows against the projected composition by element
    /// id, so a vacant Seat under a Description-visible surface renders as the
    /// published ask — and a seat under a hidden one never leaves the server. The
    /// rows themselves are raw and Total-tier (they include `occupant`);
    /// authorization/projection is the caller's concern, settled before this
    /// is read.
    async fn seats(&self, commission: CommissionId) -> anyhow::Result<Vec<Seat>>;

    /// Load the commission's **whole composition** — every tab, every widened
    /// surface mode, and every element (ZMVP-166; Flat Composition DD
    /// `45514754`). `None` if no such commission exists; a commission is never
    /// tabless (its skeleton tabs are minted with it, and backfilled for those
    /// that predate the model), so a found commission always yields a
    /// composition — an *empty* one, with tabs and no elements, being the
    /// ordinary state of a fresh commission.
    ///
    /// All three parts travel together deliberately: effective visibility is
    /// `min(tab, surface, element)`, so a load returning only elements would put
    /// every caller one forgotten lookup away from projecting unclamped content.
    ///
    /// **The returned composition is raw and server-internal** — `Total`-tier
    /// content included, deliberately not serializable (see
    /// [`CommissionComposition`], and
    /// [`ElementPayload`](crate::elements::commission::ElementPayload) for the
    /// content itself). Callers serialize only through the viewer projection
    /// ZMVP-170 introduces; authorization is the caller's concern, settled
    /// before this is read.
    ///
    /// Named for what it returns — the whole [`CommissionComposition`], not
    /// just its elements.
    async fn load_composition(
        &self,
        id: CommissionId,
    ) -> anyhow::Result<Option<CommissionComposition>>;

    /// The [`CommissionFile`] entry `key` names **within `commission`**, or `None`
    /// if this commission holds no such entry (ZMVP-88). Scoped to `commission` by
    /// construction: a `key` that belongs to a *different* commission answers
    /// `None` here, so the retrieval path never becomes a cross-commission
    /// existence oracle — a participant of one commission cannot confirm a file of
    /// another. The bytes themselves live behind
    /// [`FileStore`](crate::ports::FileStore); this read only settles the
    /// commission→file link the participant gate authorizes against.
    async fn find_file(
        &self,
        commission: CommissionId,
        key: FileKey,
    ) -> anyhow::Result<Option<CommissionFile>>;

    /// The pending [`SeatInvitation`] for `(commission, seat, user)`, or `None`
    /// if there isn't one (ZMVP-78 — the Seat mirror of
    /// [`AccountStore::find_pending_invitation`](crate::ports::AccountStore::find_pending_invitation)).
    /// Underpins the idempotent re-invite: a hit means "already invited to this
    /// seat", so the owner-invite handler returns it rather than issuing a
    /// duplicate. Only ever returns a **pending** offer — accepted/revoked
    /// invitations are history, not live offers. Scoped to `commission` **in the
    /// query itself**, not by caller discipline: a seat id from some other
    /// commission never matches, so a handler that authorized against one
    /// commission cannot reach another's offers (the closed-door rule
    /// [`CommissionStore::is_participant`] documents, enforced by construction).
    async fn find_pending_seat_invitation(
        &self,
        commission: CommissionId,
        seat: ElementId,
        user: UserId,
    ) -> anyhow::Result<Option<SeatInvitation>>;

    /// The commissions `owner` OWNS, owner-POV only (ZMVP-157) — the frontend's
    /// own-commissions list. The non-participant projection view (ZMVP-75) is a
    /// distinct, later surface this does not attempt: nothing here answers "what
    /// can I see as a seated participant", only "what do I own".
    ///
    /// **Archived commissions are excluded.** An archived commission is meant to
    /// disappear from *active views* (Deletion DD `3014657`; ZMVP-68), and
    /// [`Commission::archived_at`] documents that filtering it out is exactly a
    /// listing projection's job — this is that filter's first implementation.
    /// The record and its facts still survive and stay reachable through
    /// [`find`](Self::find) for a Participant who already has the id (e.g. to
    /// un-archive it); this list just never surfaces one to browse to.
    ///
    /// Ordered by [`CommissionId`] — UUIDv7 sorts as creation order — so the
    /// listing is deterministic; no pagination (v1 volumes are small, the same
    /// stance as ZMVP-110).
    async fn list_owned_by(&self, owner: UserId) -> anyhow::Result<Vec<Commission>>;
}

/// The error an element-writing call carries (as the source of its
/// `anyhow::Error`) when the named tab does not exist **in that commission** —
/// covering both a truly absent tab id and one belonging to some other
/// commission, indistinguishably. One answer for both by design: the closed-door
/// policy means a caller must learn nothing about other commissions from probing
/// tab ids (the same collapse [`CommissionStore::is_participant`] documents for
/// commissions themselves).
///
/// This is the *friendly* half of a rule the schema already enforces
/// structurally: `commission_element`'s **composite** foreign key
/// `(tab_id, commission_id) → commission_tab (id, commission_id)` makes an
/// element citing another commission's tab unwritable at the database. The gate
/// exists so the route can answer `404` instead of surfacing a constraint
/// violation — never as the thing standing between a caller and another
/// commission's data. Adapters return it so the route can `downcast_ref` — the
/// [`HandleTaken`](crate::ports::HandleTaken) pattern.
#[derive(Debug)]
pub struct UnknownTab;

impl std::fmt::Display for UnknownTab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tab not found in this commission")
    }
}

impl std::error::Error for UnknownTab {}

/// The error an element-writing call carries (as the source of its
/// `anyhow::Error`) when the composition [`SKELETON`](crate::elements::commission::SKELETON)
/// does not declare the named surface **in the named tab**.
///
/// **The refusal is about the pair, not the surface alone**: a surface that is
/// perfectly real under some *other* tab is refused here just as an invented
/// name is, because an element may only be contributed where the skeleton
/// actually describes a place for it.
///
/// Surfaces are **code-declared, global and invariant**: the same set exists for
/// every commission, and no write creates one. So unlike [`UnknownTab`] this
/// leaks nothing — "no tab called `main` holds a surface called `xyz`" is a fact
/// about the program, not about anyone's commission — and it is honest to answer
/// as a malformed request (`422`) rather than hiding it behind a 404. Both
/// adapters check the same const
/// ([`declares_surface`](crate::elements::commission::declares_surface)), so the
/// two cannot disagree about which addresses are real.
///
/// **Ordering note.** Because the check needs the tab's *declared name*, the
/// adapters resolve the tab **first**: an address that is wrong in both ways
/// (unknown tab *and* undeclared surface) refuses as [`UnknownTab`]. Both
/// adapters must keep that order, or the two would answer the same request
/// differently.
#[derive(Debug)]
pub struct UnknownSurface;

impl std::fmt::Display for UnknownSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "no such declared surface")
    }
}

impl std::error::Error for UnknownSurface {}

/// The error [`CommissionWrites::remove_element`] carries (as the source of its
/// `anyhow::Error`) when the addressed element does not exist **in that
/// commission** — covering both a truly absent element id and one belonging to
/// some other commission, indistinguishably (the removal twin of
/// [`UnknownTab`], and the same closed-door collapse: probing element ids
/// through a removal must reveal nothing about other commissions). Adapters
/// return it so the route can `downcast_ref` and answer `404` rather than a
/// generic `500`.
///
/// There is deliberately **no sibling "cannot remove" error**. The tree needed
/// one because its root was a removable-looking node; the flat model has no such
/// thing — tabs and surfaces are skeleton, not elements, so no element id
/// addresses one, and every element is removable by construction.
#[derive(Debug)]
pub struct ElementNotFound;

impl std::fmt::Display for ElementNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "element not found in this commission")
    }
}

impl std::error::Error for ElementNotFound {}

/// The **write** surface of Zurfur's record of commissions — reachable only on an
/// open [`UnitOfWork`](crate::ports::UnitOfWork) (`uow.commissions()`), so no
/// private-store commission write can skip a transaction (ZMVP-65; DD `24150017`).
#[async_trait]
pub trait CommissionWrites: Send {
    /// Persist a freshly created [`Commission`] — **together with its skeleton
    /// tab rows** ([`declared_tabs`](crate::elements::commission::declared_tabs),
    /// ZMVP-166) **and its owner's participant row** (ZMVP-76, Engineer
    /// ruling: participant-hood is persisted; the owner is a permanent
    /// Participant from birth), in this same write. Both are minted *inside*
    /// the implementation, not by the caller, so a tabless — or owner-less —
    /// commission is unrepresentable: no call site exists that could persist
    /// the row and forget either. One private-side write on the open unit of
    /// work.
    ///
    /// Minting the tabs at birth is the **withheld-at-birth discipline**: tab
    /// state exists explicitly from the first instant, born
    /// [`Total`](crate::elements::commission::VisibilityMode::Total), so absence
    /// of a tab row never has to *mean* anything (R4's "absence ≠ withheld").
    async fn create(&mut self, commission: &Commission) -> anyhow::Result<()>;

    /// Declare a [`NewSeat`] into a declared **surface** (ZMVP-76 AC1/AC2),
    /// atomically as **one element + one satellite row sharing the seat's id**,
    /// on the open transaction: the composition gains an ordinary element (typed
    /// [`ElementType::seat`](crate::elements::commission::ElementType::seat),
    /// empty payload, append order within the band, born `Total`) and the
    /// interpreted seat data (kind, requirements, the vacant occupant slot)
    /// lands in the `commission_seat` satellite keyed by that element's id
    /// (Gate A ruling E20). The same gates as every element write: an
    /// absent/foreign tab refuses with [`UnknownTab`], an undeclared surface
    /// with [`UnknownSurface`]. Authority (owner-only in v1) and the
    /// commission's own existence are the caller's checks. The declaration is
    /// changelog-recorded ([`SeatDeclared`]): the caller appends the matching
    /// entry through [`ChangelogWrites`](crate::ports::ChangelogWrites) **in
    /// this same unit of work**, so the seat and its record land atomically.
    ///
    /// [`SeatDeclared`]: crate::elements::commission::ChangelogEntryKind::SeatDeclared
    async fn declare_seat(&mut self, seat: &NewSeat) -> anyhow::Result<()>;

    /// Persist a freshly issued, pending [`SeatInvitation`] (ZMVP-78 — the
    /// issuing half of seat invite-then-accept; acceptance is ZMVP-79). The Seat
    /// mirror of [`AccountWrites::create_invitation`](crate::ports::AccountWrites::create_invitation):
    /// at most one *pending* invitation may exist per (seat, invited user), so if
    /// one already does this is a no-op rather than a second row — the
    /// store-level backstop for the idempotent re-invite the caller also guards
    /// by checking
    /// [`CommissionStore::find_pending_seat_invitation`](crate::ports::CommissionStore::find_pending_seat_invitation)
    /// first. Several *different* Users may hold pending invitations to one Seat
    /// (the acceptance race is ZMVP-79's to resolve); only a duplicate pending
    /// for the same (seat, user) pair is dropped. Authority (the inviter being
    /// the commission owner, the seat being vacant) is the caller's check,
    /// settled before this is reached. A private-side write, never a cross-store
    /// dual write. Deliberately **not** changelog-recorded (Engineer ruling
    /// 2026-07-16: no changelog entries in this ticket).
    async fn create_seat_invitation(&mut self, invitation: &SeatInvitation) -> anyhow::Result<()>;

    /// Transition a pending seat invitation to revoked, so it can no longer be
    /// accepted (ZMVP-78 — the Seat mirror of
    /// [`AccountWrites::revoke_invitation`](crate::ports::AccountWrites::revoke_invitation)).
    /// Idempotent on a non-pending or absent invitation — a no-op, not an error;
    /// the caller decides whether absence/already-revoked is a 404/200. *Who* may
    /// revoke (the commission owner) is the caller's authority check. A
    /// private-side write, never a cross-store dual write.
    async fn revoke_seat_invitation(&mut self, id: SeatInvitationId) -> anyhow::Result<()>;

    /// Contribute a [`NewElement`] into a declared surface (ZMVP-166; Flat
    /// Composition DD `45514754`). Order is assigned here, **on the open
    /// transaction** — append = max `position` + 1 **within
    /// `(tab, surface, band)`** — and implementations must **serialize
    /// concurrent appends** into one such group (the pg adapter locks the tab
    /// row `FOR UPDATE`; the mem fake's single lock is coarser), so concurrent
    /// adds cannot race to one position NOR abort on the position uniqueness at
    /// commit (the PR #103 hardening, carried over).
    ///
    /// The element is born [`Total`](crate::elements::commission::VisibilityMode::Total)
    /// (see [`NewElement::contributed`] — there is no mode to pass), the payload
    /// is stored semantically unmodified (it round-trips as an equal JSON value;
    /// jsonb is not byte-preserving), and the band is the placeholder
    /// [`Band::default`](crate::elements::commission::Band::default).
    ///
    /// The tab must exist in `element.commission_id`: an absent tab id *and* one
    /// belonging to another commission both fail with [`UnknownTab`] as the
    /// error source (one indistinguishable answer — see its docs; the composite
    /// foreign key makes the cross-commission case unwritable regardless). A
    /// surface the skeleton does not declare fails with [`UnknownSurface`].
    /// Authority (owner-only in v1) and the commission's own existence are the
    /// caller's checks, settled before this is reached. Deliberately **not**
    /// changelog-recorded: composition edits are not in the frozen entry
    /// taxonomy (ZMVP-87).
    async fn add_element(&mut self, element: &NewElement) -> anyhow::Result<()>;

    /// Remove `element` from the commission's composition (ZMVP-166). Elements
    /// are leaves — there is no subtree to take, and nothing can be left orphaned
    /// — so this removes exactly one row, plus whatever hangs off its identity by
    /// cascade (a Slot or Seat satellite, and a seat's pending invitations).
    /// Runs on the open transaction, which also **renumbers the remaining
    /// `(tab, surface, band)` group** (contiguous from 0, order preserved) so
    /// positions stay consistent — removal and renumber commit or roll back
    /// together.
    ///
    /// The target must exist in `commission`: an absent element id *and* one
    /// belonging to another commission both fail with [`ElementNotFound`] (one
    /// indistinguishable answer — see its docs). There is no protected element:
    /// tabs and surfaces are skeleton, not elements, so no element id addresses
    /// one. Authority (owner-only in v1) and the commission's own existence are
    /// the caller's checks, settled before this is reached. Deliberately **not**
    /// changelog-recorded (ZMVP-87's frozen taxonomy).
    ///
    /// **Plugin note (carried from ZMVP-73):** when plugin-contributed elements
    /// land, removing one must emit an event the owning plugin can observe (its
    /// signal to drop external state tied to it). No such event machinery exists
    /// yet — plugins don't — so this is a recorded need, not a hook. The
    /// per-plugin bound and the write gate are ZMVP-167's.
    async fn remove_element(
        &mut self,
        commission: CommissionId,
        element: ElementId,
    ) -> anyhow::Result<()>;

    /// Record a file entry's [`CommissionFile`] link (ZMVP-88) — the Index-canonical
    /// row tying an uploaded file's opaque [`FileKey`] to its commission. Written on
    /// the open transaction **together with** the `file_added` changelog entry the
    /// caller appends through [`ChangelogWrites`](crate::ports::ChangelogWrites), so
    /// the entry and its record land atomically (Changelog DD D4). The **bytes** are
    /// stored separately, before this unit, through
    /// [`FileStore::put`](crate::ports::FileStore::put) — never inside this
    /// transaction (blob bytes cannot ride a Postgres unit of work; orphan-on-rollback
    /// is accepted). The row is commission-owned bookkeeping, **not a
    /// [`Fact`](crate::elements::commission::Fact)**: it cascades away with the
    /// commission, so a commission with only file entries stays hard-deletable (AC2).
    async fn add_file(&mut self, file: &CommissionFile) -> anyhow::Result<()>;

    /// Declare **Slots** on the commission — a batch, all in this one write
    /// (Engineer ruling, PR #108: a commission's Slots usually arrive several
    /// at a time, so declaration is an array operation). A Slot is not a kind
    /// of element: for each [`NewSlot`] the store contributes an ordinary
    /// element (typed
    /// [`ElementType::slot`](crate::elements::commission::ElementType::slot))
    /// into the named **surface**, and persists the Slot itself — the required
    /// title and optional notes — as its satellite row, keyed by that element's
    /// id (ZMVP-77 AC1; the Slot mirror of the Seat satellite ruling, Gate A
    /// E20). The whole batch lands or none of it does: the first refused Slot
    /// aborts the write, and the open transaction takes the earlier inserts
    /// with it.
    ///
    /// The carrying element follows exactly the
    /// [`add_element`](Self::add_element) contract: append order assigned on the
    /// open transaction within `(tab, surface, band)`, an absent/foreign tab
    /// refusing with [`UnknownTab`], an undeclared surface refusing with
    /// [`UnknownSurface`], authority and commission existence settled by the
    /// caller. The element's payload is the empty object — the Slot's substance
    /// lives in the satellite, which is why the generic element add cannot
    /// declare one.
    ///
    /// **No changelog entry**: the frozen ZMVP-87 taxonomy carries
    /// `seat_declared` for Seats but no Slot variant, and the taxonomy is not
    /// this ticket's to grow (flagged to the Engineer rather than invented).
    ///
    /// Nothing here fills a Slot — no occupant is even representable on
    /// [`NewSlot`] or in its storage (AC2/AC3: an empty Slot is a valid,
    /// permanent state; the assignment surface is the Character epic's).
    async fn declare_slots(&mut self, slots: &[NewSlot]) -> anyhow::Result<()>;

    /// Whether the commission bears any [`Fact`](crate::elements::commission::Fact)
    /// — the single predicate deciding hard-delete legality (ZMVP-67; Deletion DD
    /// `3014657`). The delete/archive gates (ZMVP-66/68) consume **this port**,
    /// never ad-hoc checks.
    ///
    /// A *read*, deliberately placed on the transactional write view rather than a
    /// pool-backed store (conductor ruling E17): the gate that asks it runs in the
    /// **same transaction** as the delete it guards, so a fact minted between check
    /// and delete is unrepresentable (no TOCTOU window) — the same
    /// make-unsoundness-unreachable posture as the Unit of Work itself.
    ///
    /// An unknown commission answers `false`: absence of the commission is absence
    /// of facts, not an error — existence is the caller's separate concern. With no
    /// fact-minter wired anywhere (every fact kind — Product, rating, EXP,
    /// achievement, payment — is a future ticket), every commission answers `false`
    /// (AC3). Implementations carry the registry duty stated on
    /// [`Fact`](crate::elements::commission::Fact): every fact kind's storage must
    /// join this predicate in the same change that introduces it.
    async fn commission_has_facts(&mut self, id: CommissionId) -> anyhow::Result<bool>;

    /// **Hard-delete** the commission: remove its row, taking every child row
    /// with it (ZMVP-66; Deletion DD `3014657` — "Delete = hard delete, possible
    /// only while fact-free"). In the pg adapter the children reap via each child
    /// table's `ON DELETE CASCADE` (the epic's convention, ruling E35) — at this
    /// stack that is `commission_changelog`, the commission's own memory, which
    /// dies with it by design (DD retention); the mem adapter mirrors the
    /// cascade. Every child row is a non-fact **by construction**: the caller
    /// gates this on [`commission_has_facts`](Self::commission_has_facts) in the
    /// **same unit of work** (ruling E17 — no TOCTOU window), so the cascade can
    /// never take a fact with it.
    ///
    /// Authority (owner-only) and existence are the caller's checks, settled
    /// before this is reached; deleting an absent commission matches no row and
    /// is a no-op, which keeps a lost race (a concurrent delete) idempotent
    /// rather than an error.
    async fn delete(&mut self, id: CommissionId) -> anyhow::Result<()>;

    /// Archive (`Some(when)`) or un-archive (`None`) the commission — the soft
    /// path of the Deletion DD (`3014657`): the record and its facts survive
    /// intact, only the active-view listings lose it (ZMVP-68). Returns whether
    /// the state actually **transitioned** (active→archived or
    /// archived→active); a repeat in the same direction changes nothing,
    /// answers `false`, and keeps the original archive stamp — the caller keys
    /// its matching [`Archived`]/[`Unarchived`] changelog append on this bool
    /// **in the same unit of work**, so a duplicate entry is unrepresentable,
    /// not merely unlikely (the no-TOCTOU posture of
    /// [`commission_has_facts`](Self::commission_has_facts)). An absent
    /// commission matches nothing and answers `false`; existence and authority
    /// (owner-only in both directions — Engineer ruling 2026-07-05 on ZMVP-68)
    /// are the caller's checks.
    ///
    /// [`Archived`]: crate::elements::commission::ChangelogEntryKind::Archived
    /// [`Unarchived`]: crate::elements::commission::ChangelogEntryKind::Unarchived
    async fn set_archived(
        &mut self,
        id: CommissionId,
        archived_at: Option<DateTimeUtc>,
    ) -> anyhow::Result<bool>;

    /// Set — or replace — the commission's maturity posture (ZMVP-31;
    /// Maturity Vocabulary DD `29982722`): the four-tier rating plus the
    /// orthogonal Graphic flag, one envelope write on the open unit of work.
    ///
    /// **Replace-only, deliberately**: the signature takes a [`Maturity`],
    /// not an `Option`, so no call site can clear a rating back to unrated —
    /// a commission widened past Private (which ZMVP-74 gates on a rating
    /// being present) can therefore never *lose* its rating through this
    /// port; the unrated state exists only between birth and the first
    /// rating. Setting on an absent commission is a no-op write; existence
    /// and authority (owner-only in v1) are the caller's checks, settled
    /// before this is reached. Deliberately **not** changelog-recorded:
    /// maturity edits are not in the frozen entry taxonomy (ZMVP-87).
    async fn set_maturity(&mut self, id: CommissionId, maturity: Maturity) -> anyhow::Result<()>;

    /// Set (`Some`) or clear (`None`) the commission's external **linked
    /// channel** pointer (ZMVP-87 AC3; Changelog DD Decision 2). Returns whether
    /// the stored value actually **changed**; a write that repeats the current
    /// state (re-linking the same pointer, clearing an already-clear channel)
    /// touches nothing and answers `false`. The declaration is
    /// changelog-recorded, so the caller keys its matching
    /// `channel_linked`/`channel_unlinked` append through
    /// [`ChangelogWrites`](crate::ports::ChangelogWrites) on this bool **in this
    /// same unit of work** — the pointer and its record land atomically (DD D4)
    /// and a no-change entry is unrepresentable even under concurrent writers
    /// (the same no-TOCTOU posture as
    /// [`commission_has_facts`](Self::commission_has_facts)). An absent
    /// commission matches nothing and answers `false`; existence and authority
    /// (owner-only in v1) are the caller's checks, settled before this is
    /// reached.
    async fn set_linked_channel(
        &mut self,
        id: CommissionId,
        channel: Option<&ChannelPointer>,
    ) -> anyhow::Result<bool>;

    /// **Place** the commission into `account`'s position (ZMVP-70; Ownership
    /// Separation DD `29130754` Decision 1/6): append one row to the append-only
    /// placement log **and** repoint the denormalized current-placement pointer to
    /// it, atomically on the open unit — so the cached pointer equals the latest
    /// log row after every (re)placement, by construction (no second transaction).
    /// Re-placement always appends; the log is never rewritten (AC2). The
    /// commission and the account must exist — the caller settles that first (a
    /// commission owner-only act; the account resolved to a live row) — and the FK
    /// onto `commission`/`account` is the store-level backstop. Placement confers
    /// **no** in-commission authority (Decision 8) and — deliberately — appends no
    /// changelog entry (the placement log *is* the record; the Changelog DD
    /// taxonomy has no placement variant). A private-side write, never a
    /// cross-store dual write.
    async fn place(
        &mut self,
        commission: CommissionId,
        account: AccountId,
        placed_by: UserId,
        at: DateTimeUtc,
    ) -> anyhow::Result<()>;

    /// Issue `account` a **view grant** — a key to see `commission` at `level`
    /// (ZMVP-70; Ownership Separation DD `29130754` Decision 3). At most one key
    /// per (commission, account): re-granting **replaces** the level (upsert —
    /// "issuing anew", Decision 5, no soft-deleted rows). The grant row is a *pure
    /// key* (just the level); who issued it and when live only in the changelog
    /// (Decision 5), so the caller settles authority first (owner-only in v1;
    /// Admin-capable once ZMVP-83 lands) and appends the [`ViewGrantIssued`]
    /// changelog entry — carrying the actor — in **this same unit of work** (issue
    /// is a recorded-but-not-broadcast fact), so the key and its record land
    /// atomically. A key only lifts (Decision 4) and never makes the account's
    /// members Participants (Decision 8). A private-side write, never a cross-store
    /// dual write.
    ///
    /// [`ViewGrantIssued`]: crate::elements::commission::ChangelogEntryKind::ViewGrantIssued
    async fn grant_view(
        &mut self,
        commission: CommissionId,
        account: AccountId,
        level: GrantLevel,
    ) -> anyhow::Result<()>;

    /// **Revoke** `account`'s view grant on `commission` — **hard-delete** the key
    /// row (ZMVP-70; Ownership Separation DD `29130754` Decision 5). Because
    /// visibility is enforced server-side at serialization, revocation is effective
    /// on the next render by construction — there is no session to invalidate.
    /// Returns whether a key was actually removed: revoking an account that holds
    /// none is an idempotent no-op answering `false`, so the caller keys its
    /// [`ViewGrantRevoked`] changelog append on a real transition **in the same
    /// unit of work** (the no-duplicate-entry posture of
    /// [`set_archived`](Self::set_archived)). A private-side write, never a
    /// cross-store dual write.
    ///
    /// [`ViewGrantRevoked`]: crate::elements::commission::ChangelogEntryKind::ViewGrantRevoked
    async fn revoke_view(
        &mut self,
        commission: CommissionId,
        account: AccountId,
    ) -> anyhow::Result<bool>;

    /// Set (`Some`) or clear (`None`) the commission's **direction-axis
    /// Status** (ZMVP-85; DESIGN/Commission, Status). One nullable cell
    /// (ruling E29): a set REPLACES whatever value is held — axis exclusivity
    /// by construction, never by check. This is the column's **only writer**:
    /// direction transitions are always an explicit Participant act (Engineer
    /// ruling 2026-07-01), so no content event or system sweep may reach for
    /// it. The change is changelog-recorded — the caller appends the matching
    /// `status_changed` entry through
    /// [`ChangelogWrites`](crate::ports::ChangelogWrites) **in this same unit
    /// of work** (Changelog DD D4). Setting on an absent commission is a no-op
    /// write; existence and authority (any Participant) are the caller's
    /// checks, settled before this is reached. Returns `true` iff the stored
    /// value actually changed (`… IS DISTINCT FROM`), so the caller appends the
    /// `status_changed` entry only on a real change — never a spurious entry
    /// when the value already held is re-set, even under a concurrent racing
    /// write (the [`set_linked_channel`](Self::set_linked_channel) contract).
    async fn set_direction_status(
        &mut self,
        id: CommissionId,
        status: Option<DirectionStatus>,
    ) -> anyhow::Result<bool>;

    /// Set (`Some`) or clear (`None`) the commission's **deadline** — the
    /// nullable-but-fixed envelope field (ZMVP-86 AC1; DESIGN/Commission). A
    /// Participant act: the caller appends the matching
    /// `deadline_set`/`deadline_extended` entry through
    /// [`ChangelogWrites`](crate::ports::ChangelogWrites) **in this same unit
    /// of work** (Changelog DD D4), and owns the axis recompute (a deadline
    /// no longer passed clears Late; a cleared deadline wipes the axis via
    /// [`set_deadline_status`](Self::set_deadline_status) — AC4). Setting on
    /// an absent commission is a no-op write; existence and authority (any
    /// Participant) are the caller's checks, settled before this is reached.
    /// Returns `true` iff the stored value actually changed (`… IS DISTINCT
    /// FROM`), so the caller appends the changelog entry only on a real change
    /// — never a spurious entry when the deadline already held is re-set, even
    /// under a concurrent racing write (mirrors [`set_direction_status`](Self::set_direction_status)).
    async fn set_deadline(
        &mut self,
        id: CommissionId,
        deadline: Option<DateTimeUtc>,
    ) -> anyhow::Result<bool>;

    /// Set (`Some`) or clear (`None`) the commission's **deadline-axis
    /// Status** (ZMVP-86). One nullable cell (ruling E29): a set REPLACES the
    /// value held — axis exclusivity by construction. Exactly **two writers**
    /// exist, both changelog-recorded in the same unit of work (DD D4): the
    /// deadline-status endpoint (the Participant's manual
    /// [`Delayed`](DeadlineStatus::Delayed) flag and the deadline write's axis
    /// recompute) and the deadline sweeper (the system
    /// [`Late`](DeadlineStatus::Late) mark — the only system writer, scoped to
    /// this axis and nothing else). The caller holds AC4: never leave a value
    /// on a commission without a deadline. Setting on an absent commission is
    /// a no-op write. Returns `true` iff the stored value actually changed
    /// (`… IS DISTINCT FROM`), so the caller records the changelog entry only
    /// on a real change (mirrors [`set_direction_status`](Self::set_direction_status)).
    async fn set_deadline_status(
        &mut self,
        id: CommissionId,
        status: Option<DeadlineStatus>,
    ) -> anyhow::Result<bool>;

    /// The commissions the deadline sweeper must mark Late **as of `now`**
    /// (ZMVP-86 AC2, ruling E12): deadline strictly before `now`, not already
    /// [`Late`](DeadlineStatus::Late), lifecycle not terminal
    /// ([`LifecycleStep::is_terminal`](crate::elements::commission::LifecycleStep::is_terminal)
    /// — a closed commission's missed deadline is history, not lateness).
    /// Commissions without a deadline never appear (AC4). Ordered by deadline
    /// for determinism.
    ///
    /// A *read*, deliberately on the transactional write view (the
    /// [`commission_has_facts`](Self::commission_has_facts) posture, ruling
    /// E17): the sweeper scans and marks **in one unit of work**, so nothing
    /// can slip between the scan and the mark — one sweep, one transaction.
    async fn lapsed_deadlines(&mut self, now: DateTimeUtc) -> anyhow::Result<Vec<LapsedDeadline>>;
}
