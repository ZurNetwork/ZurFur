//! Commission ports: the canonical [`CommissionStore`] read surface and the
//! [`CommissionWrites`] write view (ZMVP-65/67/87). Commissions are entirely
//! Index-side — nothing on these surfaces ever touches atproto.

use async_trait::async_trait;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId,
        commission::{
            ChannelPointer, Commission, CommissionId, CommissionTree, GrantLevel, NewSurface,
            Placement,
        },
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
    /// transaction** — append = max sibling `position` + 1 — so concurrent adds
    /// can't race to one slot (Tree Storage DD `28409880` Decision 3). The
    /// parent must exist in `surface.commission_id`'s tree: an absent parent
    /// *or* one belonging to another commission fails with
    /// [`ParentNodeNotFound`] as the error source (one indistinguishable
    /// answer — see its docs). Authority (owner-only in v1) and the
    /// commission's own existence are the caller's checks, settled before this
    /// is reached. Deliberately **not** changelog-recorded: tree edits are not
    /// in the frozen entry taxonomy (ZMVP-87).
    async fn add_surface(&mut self, surface: &NewSurface) -> anyhow::Result<()>;

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
}
