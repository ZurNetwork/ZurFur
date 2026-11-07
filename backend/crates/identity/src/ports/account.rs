use async_trait::async_trait;

use crate::elements::account::{Account, AccountFacts, AccountId, AccountMembership, ListingScope};
use crate::elements::handle::Handle;
use crate::elements::invitation::{Invitation, InvitationId};
use crate::elements::role::Role;
use crate::elements::user::UserId;
use crate::elements::user_account::UserAccount;
use kernel::datetime::DateTimeUtc;
use kernel::elements::did::Did;

/// The **read** surface of Zurfur's record of accounts and who owns them —
/// pool-backed and non-transactional; every write lives on [`AccountWrites`].
#[async_trait]
pub trait AccountStore: Send + Sync {
    /// The live account `id` names, or `None` if absent or soft-deleted.
    async fn find(&self, id: &AccountId) -> anyhow::Result<Option<Account>>;

    /// The role a user holds in an account, or `None` if they are not a member.
    async fn role_of(&self, user: &UserId, account: &AccountId) -> anyhow::Result<Option<Role>>;

    /// The pending invitation for `(account, invited_user)`, or `None`. Only ever
    /// a pending offer; accepted/revoked ones are history. Underpins the
    /// idempotent re-invite.
    async fn find_pending_invitation(
        &self,
        account: &AccountId,
        invited_user: &UserId,
    ) -> anyhow::Result<Option<Invitation>>;

    /// Resolve an [`InvitationId`] to its [`Invitation`] in any state, or `None`.
    async fn find_invitation(&self, id: &InvitationId) -> anyhow::Result<Option<Invitation>>;

    /// The sovereign [`Did`] of the account holding `handle`, live or deactivated,
    /// or `None`. Exact-match. Backs atproto handle resolution and the founding-time
    /// duplicate check.
    async fn find_did_by_handle(&self, handle: &Handle) -> anyhow::Result<Option<Did>>;

    /// How many handle changes `account` has recorded on or after `since`. The
    /// rate limit and its window are the caller's policy.
    async fn count_handle_changes_since(
        &self,
        account: &AccountId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64>;

    /// Whether `handle` was vacated by some account other than `excluding` on or
    /// after `since`, and so is still quarantined to it. `excluding` lets an
    /// account reclaim its own just-vacated handle.
    async fn handle_reserved_for_other(
        &self,
        handle: &Handle,
        excluding: Option<&AccountId>,
        since: DateTimeUtc,
    ) -> anyhow::Result<bool>;

    /// Every live account `user` holds a role in, each paired with that role.
    /// Soft-deleted accounts excluded; ordered by [`AccountId`], unpaginated.
    ///
    /// `scope` is required so it cannot be forgotten: rendering one user's
    /// memberships to another must pass
    /// [`PublicProfile`](ListingScope::PublicProfile), which applies the
    /// `listed_on_profile` privacy valve.
    async fn list_for_user(
        &self,
        user: &UserId,
        scope: ListingScope,
    ) -> anyhow::Result<Vec<AccountMembership>>;

    /// Every soft-deleted (deactivated) account in which `owner` holds the
    /// Owner role, ordered by [`AccountId`], unpaginated. The one read that
    /// returns deactivated accounts; the caller passes the viewer's own id.
    async fn list_deactivated_for_owner(&self, owner: &UserId) -> anyhow::Result<Vec<Account>>;
}

/// The **write** surface of Zurfur's record of accounts and memberships —
/// reachable only on an open `UnitOfWork` (`uow.accounts()`). Authorization is
/// always the caller's, settled before any of these is reached.
#[async_trait]
pub trait AccountWrites: Send {
    /// Persist a freshly founded account together with its founder's Owner
    /// membership, atomically. A handle collision fails with [`HandleTaken`] as
    /// the error source; anything else is an opaque store error.
    async fn create(&mut self, account: &Account, owner: &UserAccount) -> anyhow::Result<()>;

    /// Repoint `accounts.handle` to `new` and append the audit-log row that
    /// rate-limits future changes and quarantines `old`, atomically at `at`.
    /// `old` is an optimistic-concurrency precondition: a soft-deleted account, or
    /// one whose handle moved under a concurrent rename, fails and writes no audit
    /// row. A collision fails with [`HandleTaken`]. The public half — the DID
    /// document's `alsoKnownAs` — is a separate retryable step the caller runs
    /// first, never inside this transaction.
    async fn change_handle(
        &mut self,
        account: &AccountId,
        old: &Handle,
        new: &Handle,
        at: DateTimeUtc,
    ) -> anyhow::Result<()>;

    /// Change the role of an existing member. Never seats anyone, never grants
    /// Owner, never changes the Owner's role. Fails, writing nothing, when the pair
    /// holds no membership, when the member is the Owner, or when `role` is Owner.
    /// Idempotent for a member. Fails with [`AccountDeactivated`](crate::ports::AccountDeactivated) when the Account is
    /// deactivated.
    async fn grant_role(
        &mut self,
        user: &UserId,
        account: &AccountId,
        role: &Role,
    ) -> anyhow::Result<()>;

    /// Remove a non-Owner member's membership: revokes the invitations they had
    /// issued, re-homes their role-tree children to their own parent, and deletes the
    /// membership, in one transaction. Fails, writing nothing, when the member is the
    /// Owner. Idempotent on a non-member. Passes on a deactivated Account, as a
    /// departure does; refusing it there is the caller's.
    async fn revoke_role(&mut self, user: &UserId, account: &AccountId) -> anyhow::Result<()>;

    /// A member leaves their own account: the same store effects and the same Owner
    /// refusal as [`revoke_role`](AccountWrites::revoke_role), self-initiated so no
    /// rank check applies. Passes on a deactivated Account.
    async fn leave(&mut self, user: &UserId, account: &AccountId) -> anyhow::Result<()>;

    /// Persist a freshly issued, pending [`Invitation`]. At most one pending
    /// invitation may exist per (account, invited user); a duplicate is a no-op
    /// rather than a second row. Fails, writing nothing, when the inviter isn't a
    /// member of the account. Fails with [`AccountDeactivated`](crate::ports::AccountDeactivated) when the Account is
    /// deactivated.
    async fn create_invitation(&mut self, invitation: &Invitation) -> anyhow::Result<Invitation>;

    /// Transition a pending invitation to revoked. Idempotent on a non-pending or
    /// absent invitation — a no-op, not an error. Passes on a deactivated Account, as
    /// a decline does; refusing an issuer's revoke there is the caller's.
    async fn revoke_invitation(&mut self, id: &InvitationId) -> anyhow::Result<()>;

    /// Accept a pending invitation: in one transaction flip it to Accepted and seat
    /// the invited User with `parent = inviter` and their `listed_on_profile`
    /// choice. The guard is atomic with the seat — a lost race seats no member.
    /// Records the Account's member fact in the same unit, at the flip's instant;
    /// an Account that already holds it keeps its first time. Fails with
    /// [`AccountDeactivated`](crate::ports::AccountDeactivated) when the Account is deactivated.
    async fn accept_invitation(
        &mut self,
        invitation: Invitation,
        listed_on_profile: bool,
    ) -> anyhow::Result<UserAccount>;

    /// Transfer ownership to another existing member, in one transaction: the
    /// incoming member becomes the sole `Owner` with no parent, the outgoing Owner
    /// is demoted to `Admin` under them. Its own seam because ownership is
    /// singular — `grant_role` never grants Owner. Fails with [`AccountDeactivated`](crate::ports::AccountDeactivated)
    /// when the Account is deactivated.
    async fn transfer_ownership(
        &mut self,
        old_owner: &UserId,
        new_owner: &UserId,
        account: &AccountId,
    ) -> anyhow::Result<()>;

    /// Soft-delete an account: stamp `deleted_at`, removing nothing. The handle
    /// stays reserved and the `did:plc` stays live, but reads treat the account as
    /// absent; memberships and invitations are kept so reactivation restores it
    /// intact. The caller holds the row from
    /// [`find_for_update`](AccountReads::find_for_update) and has read the
    /// [`facts`](AccountReads::facts) in the same unit. Idempotent.
    async fn soft_delete(&mut self, account: &AccountId) -> anyhow::Result<()>;

    /// Hard-delete an empty account: remove its invitation, membership and account
    /// rows in one unit of work, freeing the handle. Its boards cascade away; the
    /// commissions on them survive (User-owned) and the custody keys are left in
    /// place. Tombstoning the DID is a separate retryable step. The caller holds the
    /// row from [`find_for_update`](AccountReads::find_for_update) and has read the
    /// [`facts`](AccountReads::facts) in the same unit. Fails, ending the unit, on a
    /// deactivated Account and on one holding a fact. On an absent Account the
    /// PostgreSQL store fails and the in-memory store succeeds, removing nothing.
    async fn hard_delete(&mut self, account: &AccountId) -> anyhow::Result<()>;
}

/// The **read** side of an account unit of work — [`AccountStore`]'s lookups
/// on the unit's own connection (sees the unit's writes, may hold row locks).
#[async_trait]
pub trait AccountReads: Send {
    /// The live account `id` names, or `None`; as [`AccountStore::find`].
    async fn find(&mut self, id: &AccountId) -> anyhow::Result<Option<Account>>;

    /// [`find`](Self::find), with the row locked (`FOR NO KEY UPDATE`) until commit.
    async fn find_for_update(&mut self, id: &AccountId) -> anyhow::Result<Option<Account>>;

    /// [`find`](Self::find), with the row locked `FOR SHARE` until commit: other
    /// writers of the Account's members and invitations proceed, while its update or
    /// delete waits. `None` for an absent or deactivated Account.
    async fn find_for_share(&mut self, id: &AccountId) -> anyhow::Result<Option<Account>>;

    /// The facts Account `id` has reached, read on the unit's own connection.
    async fn facts(&mut self, id: &AccountId) -> anyhow::Result<AccountFacts>;

    /// `user`'s role in `account`, or `None`; as [`AccountStore::role_of`].
    async fn role_of(&mut self, user: &UserId, account: &AccountId)
    -> anyhow::Result<Option<Role>>;
}

/// An account unit of work: [`AccountReads`] + [`AccountWrites`] on one
/// connection. Vended by `UnitOfWork::accounts`.
pub trait AccountRepo: AccountReads + AccountWrites {}

impl<T: AccountReads + AccountWrites> AccountRepo for T {}
