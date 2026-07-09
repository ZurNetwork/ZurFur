//! Commission ports: the canonical [`CommissionStore`] read surface and the
//! [`CommissionWrites`] write view (ZMVP-65/67/87). Commissions are entirely
//! Index-side — nothing on these surfaces ever touches atproto.

use async_trait::async_trait;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId,
        commission::{
            ChannelPointer, Commission, CommissionId, CommissionTree, DeadlineStatus,
            DirectionStatus, GrantLevel, LapsedDeadline, NewComponent, NewSurface, NodeId,
            Placement,
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
    /// **Owner-arm only in v1**: the owner IS a Participant without holding any
    /// Seat (DESIGN/Commission — "a commission has at least one Participant: its
    /// owner, who is permanent"), and no other way in exists yet. ZMVP-79 extends
    /// the *implementations* with the seated arm behind this same signature; the
    /// participant-persistence model (a `commission_participant` table) is an
    /// open Engineer fork recorded there — do not grow a second predicate.
    ///
    /// An unknown commission has no participants, so it answers `false` — which
    /// is what lets a caller collapse "absent" and "hidden" into one uniform 404
    /// (the closed-door policy: existence is never leaked to outsiders).
    async fn is_participant(&self, commission: CommissionId, user: UserId) -> anyhow::Result<bool>;

    /// Load the commission's **whole content tree** — one indexed read of every
    /// node row, assembled into the nested [`CommissionTree`] in Rust (ZMVP-71;
    /// Tree Storage DD `28409880` Decision 4). `None` if no such commission
    /// exists; a commission is never treeless (its root is minted with it and
    /// backfilled for those that predate the tree), so a found commission always
    /// yields a tree. Corrupt row sets (no/multiple roots, detached nodes)
    /// surface as errors, never a partial tree.
    ///
    /// **The returned tree is raw and server-internal** — `Total`-tier content
    /// included, deliberately not serializable (see [`CommissionTree`]). Callers
    /// serialize only through the viewer projection ZMVP-75 introduces;
    /// authorization is the caller's concern, settled before this is read.
    async fn load_tree(&self, id: CommissionId) -> anyhow::Result<Option<CommissionTree>>;
}

/// The error an [`CommissionWrites::add_surface`] failure carries (as the source
/// of its `anyhow::Error`) when the named parent node does not exist **in that
/// commission** — covering both a truly absent node id and a node that belongs
/// to some other commission's tree, indistinguishably. One answer for both by
/// design: the closed-door policy means a caller must learn nothing about other
/// commissions' trees from probing parent ids (the same collapse
/// [`CommissionStore::is_participant`] documents for commissions themselves).
/// Adapters return it so the route can `downcast_ref` and answer `404` rather
/// than a generic `500` — the [`HandleTaken`](crate::ports::HandleTaken) pattern.
#[derive(Debug)]
pub struct ParentNodeNotFound;

impl std::fmt::Display for ParentNodeNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parent node not found in this commission")
    }
}

impl std::error::Error for ParentNodeNotFound {}

/// The error a tree-growing write carries (as the source of its
/// `anyhow::Error`) when the named parent node **exists in the caller's own
/// commission** but is a component, not a surface (ZMVP-72): components are
/// leaves — always the child of a surface, never with children — so nothing
/// grows under one. Distinct from [`ParentNodeNotFound`] and deliberately
/// reachable **only past** it: a parent outside the caller's tree always
/// answers not-found first, so this error never confirms what a foreign node
/// is. Adapters return it so the route can `downcast_ref` and answer an honest
/// `409` (the caller owns the commission and already sees the node).
#[derive(Debug)]
pub struct ParentNotASurface;

impl std::fmt::Display for ParentNotASurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the parent node is a component, not a surface")
    }
}

impl std::error::Error for ParentNotASurface {}

/// The error [`CommissionWrites::remove_node`] carries (as the source of its
/// `anyhow::Error`) when the addressed node does not exist **in that
/// commission** — covering both a truly absent node id and a node that belongs
/// to some other commission's tree, indistinguishably (the removal twin of
/// [`ParentNodeNotFound`], and the same closed-door collapse: probing node ids
/// through a removal must reveal nothing about other commissions' trees —
/// not even that a foreign node is a root, which is why this always answers
/// **before** [`CannotRemoveRoot`] can). Adapters return it so the route can
/// `downcast_ref` and answer `404` rather than a generic `500`.
#[derive(Debug)]
pub struct NodeNotFound;

impl std::fmt::Display for NodeNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "node not found in this commission")
    }
}

impl std::error::Error for NodeNotFound {}

/// The error [`CommissionWrites::remove_node`] carries (as the source of its
/// `anyhow::Error`) when the addressed node is the commission's **root
/// surface** (ZMVP-73 AC3): the root is the fixed skeleton — every commission
/// has exactly one, minted with it — so pruning it is refused outright.
/// Deliberately reachable **only past** [`NodeNotFound`]: a root in someone
/// else's commission answers not-found first, so this error never confirms
/// what a foreign node is. (The Title needs no sibling error: it is a
/// `commission` envelope field, not a tree node, so no node id addresses it —
/// irremovable by construction.) Adapters return it so the route can
/// `downcast_ref` and answer an honest `409` (the caller owns the commission
/// and already sees the root).
#[derive(Debug)]
pub struct CannotRemoveRoot;

impl std::fmt::Display for CannotRemoveRoot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "the root surface cannot be removed")
    }
}

impl std::error::Error for CannotRemoveRoot {}

/// The **write** surface of Zurfur's record of commissions — reachable only on an
/// open [`UnitOfWork`](crate::ports::UnitOfWork) (`uow.commissions()`), so no
/// private-store commission write can skip a transaction (ZMVP-65; DD `24150017`).
#[async_trait]
pub trait CommissionWrites: Send {
    /// Persist a freshly created [`Commission`] — **together with its root
    /// surface** ([`RootSurface::of`](crate::elements::commission::RootSurface::of)),
    /// in this same write (ZMVP-71 AC1). The root is minted *inside* the
    /// implementation, not by the caller, so a treeless commission is
    /// unrepresentable: no call site exists that could persist the row and
    /// forget the root. One private-side write on the open unit of work.
    async fn create(&mut self, commission: &Commission) -> anyhow::Result<()>;

    /// Grow the commission's tree: persist a [`NewSurface`] under its parent
    /// (ZMVP-71 AC2). Sibling order is assigned here, **on the open
    /// transaction** — append = max sibling `position` + 1 — and
    /// implementations must **serialize same-parent appends** (the pg adapter
    /// locks the parent row `FOR UPDATE`; the mem fake's single lock is
    /// coarser), so concurrent adds cannot race to one slot NOR abort on the
    /// slot uniqueness at commit (Tree Storage DD `28409880` Decision 3;
    /// hardened per the PR #103 review). The birth mode is inherited from the
    /// parent (see [`NewSurface::under`]). The parent must exist in
    /// `surface.commission_id`'s tree as a surface: an absent parent, one
    /// belonging to another commission, *or* a modeless node all fail with
    /// [`ParentNodeNotFound`] as the error source (one indistinguishable
    /// answer — see its docs); a parent that exists there but is a component
    /// fails with [`ParentNotASurface`] (components never have children —
    /// ZMVP-72). Authority (owner-only in v1) and the commission's own
    /// existence are the caller's checks, settled before this is reached.
    /// Deliberately **not** changelog-recorded: tree edits are not in the
    /// frozen entry taxonomy (ZMVP-87).
    async fn add_surface(&mut self, surface: &NewSurface) -> anyhow::Result<()>;

    /// Grow the commission's tree with a leaf: persist a [`NewComponent`] under
    /// its parent **surface** (ZMVP-72 AC1). The same contract as
    /// [`add_surface`](Self::add_surface) — append sibling order assigned on
    /// the open transaction, an absent/foreign parent refusing with
    /// [`ParentNodeNotFound`], a component parent refusing with
    /// [`ParentNotASurface`], authority and commission existence settled by the
    /// caller, and no changelog entry (tree edits are not in the frozen
    /// taxonomy) — plus the leaf's own half: the row stores **no mode**
    /// (a component projects with its parent) and the opaque payload
    /// semantically unmodified — round-trips as an equal JSON value (jsonb is not byte-preserving) (AC3).
    async fn add_component(&mut self, component: &NewComponent) -> anyhow::Result<()>;

    /// Prune the commission's tree: remove `node` **and its entire subtree**
    /// (ZMVP-73) — removal is subtree-deep, so a surface takes every
    /// descendant (nested surfaces and components) with it, while a component,
    /// being a leaf, goes singly. Runs on the open transaction, which also
    /// **renumbers the remaining sibling group** (contiguous from 0, order
    /// preserved) so positions stay consistent — prune and renumber commit or
    /// roll back together.
    ///
    /// The target must exist in `commission`'s own tree: an absent node id
    /// *and* a node belonging to another commission fail with [`NodeNotFound`]
    /// as the error source (one indistinguishable answer — see its docs); the
    /// root surface refuses with [`CannotRemoveRoot`], reachable only past
    /// that gate (AC3; the Title is not a node, so it is irremovable by
    /// construction). Authority (owner-only in v1) and the commission's own
    /// existence are the caller's checks, settled before this is reached.
    /// Deliberately **not** changelog-recorded: tree edits are not in the
    /// frozen entry taxonomy (ZMVP-87).
    ///
    /// **Plugin note (ZMVP-73):** when plugin-owned subtrees land, a plugin's
    /// append point is a node like any other — removing it removes the
    /// plugin's whole subtree through this same path. That removal must then
    /// emit an event the owning plugin can observe (its signal to drop
    /// external state tied to the subtree). No such event machinery exists
    /// yet — plugins don't — so this is a recorded need, not a hook.
    async fn remove_node(&mut self, commission: CommissionId, node: NodeId) -> anyhow::Result<()>;

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
    /// Status** (ZMVP-85; DESIGN/Commission, Status). One nullable slot
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
    async fn set_deadline(
        &mut self,
        id: CommissionId,
        deadline: Option<DateTimeUtc>,
    ) -> anyhow::Result<()>;

    /// Set (`Some`) or clear (`None`) the commission's **deadline-axis
    /// Status** (ZMVP-86). One nullable slot (ruling E29): a set REPLACES the
    /// value held — axis exclusivity by construction. Exactly **two writers**
    /// exist, both changelog-recorded in the same unit of work (DD D4): the
    /// deadline-status endpoint (the Participant's manual
    /// [`Delayed`](DeadlineStatus::Delayed) flag and the deadline write's axis
    /// recompute) and the deadline sweeper (the system
    /// [`Late`](DeadlineStatus::Late) mark — the only system writer, scoped to
    /// this axis and nothing else). The caller holds AC4: never leave a value
    /// on a commission without a deadline. Setting on an absent commission is
    /// a no-op write.
    async fn set_deadline_status(
        &mut self,
        id: CommissionId,
        status: Option<DeadlineStatus>,
    ) -> anyhow::Result<()>;

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
