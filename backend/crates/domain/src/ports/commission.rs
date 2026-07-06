//! Commission ports: the canonical [`CommissionStore`] read surface and the
//! [`CommissionWrites`] write view (ZMVP-65/67/87). Commissions are entirely
//! Index-side — nothing on these surfaces ever touches atproto.

use async_trait::async_trait;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChannelPointer, Commission, CommissionId},
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
    /// from *active views* — the listing projections, which filter on
    /// [`Commission::archived_at`] — never from its Participants' reach (the
    /// record and its facts survive and stay queryable, and the owner resolves
    /// it here to un-archive it).
    async fn find(&self, id: CommissionId) -> anyhow::Result<Option<Commission>>;

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
}

/// The **write** surface of Zurfur's record of commissions — reachable only on an
/// open [`UnitOfWork`](crate::ports::UnitOfWork) (`uow.commissions()`), so no
/// private-store commission write can skip a transaction (ZMVP-65; DD `24150017`).
#[async_trait]
pub trait CommissionWrites: Send {
    /// Persist a freshly created [`Commission`] as one private-side write.
    async fn create(&mut self, commission: &Commission) -> anyhow::Result<()>;

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
}
