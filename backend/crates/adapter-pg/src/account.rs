//! [`AccountStore`] (reads) and [`AccountWrites`] (writes) over PostgreSQL:
//! accounts and their memberships in the `accounts` / `account_members` tables.
//! Reads are pool-backed; writes are reachable only on an open [`UnitOfWork`]
//! (`uow.accounts()`), so no account write can skip a transaction. See ZMVP-14,
//! DESIGN/Account, and DD `24150017` (compile-enforced Unit of Work).
//!
//! The SQL lives in `queries/account/`; the typed functions and row shapes are
//! generated against the migrated schema (see [`crate::queries`]).
//!
//! [`UnitOfWork`]: domain::ports::UnitOfWork

use chrono::Utc;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::{Account, AccountId, AccountName},
        did::Did,
        handle::Handle,
        invitation::{Invitation, InvitationId, InvitationState},
        role::Role,
        user::UserId,
        user_account::UserAccount,
    },
    ports::{AccountStore, AccountWrites, HandleTaken},
};
use sqlx::{PgConnection, PgPool};

use crate::queries::account as sql;

/// THE ACCOUNT-FACT REGISTRY (ZMVP-57; Account Deletion DD `23003138`): the tables
/// whose rows are **account-anchored facts** — evidence that would be *orphaned* by
/// removing the account, so an account bearing one is **soft-deleted** (row kept,
/// handle stays reserved, `did:plc` stays live) and never hard-deleted. The
/// `account_has_facts` seam in `api/src/routes/accounts.rs` must query **every**
/// table listed here.
///
/// It is **empty by design**: no account-anchored fact store exists yet. The
/// enumeration of the fact classes is owned by the Account Deletion DD, not this
/// list — commissions are conspicuously **not** among them (they are User-owned and
/// survive account deletion, Ownership Separation DD `29130754`), and they carry no
/// foreign key onto `accounts` at all, so they can never enter the account-fact
/// scan.
///
/// Registering a table here is a **deliberate act with teeth**: the schema tripwire
/// test (`adapter-pg/tests/account.rs`) fails the moment a migration adds an
/// `accounts`-referencing table classified in neither this list nor
/// [`ACCOUNT_NON_FACT_TABLES`], and the compile-time guards below (here and beside
/// `account_has_facts` in the `api` crate) refuse to build while this list is
/// non-empty but that seam still returns its unexamined constant `false`. A future
/// account-fact minter therefore wires its storage into the seam in the same change
/// that creates it — it cannot merge past either trip by accident.
pub const ACCOUNT_FACT_TABLES: &[&str] = &[];

/// Tables that hold a foreign key onto `accounts(id)` but whose rows are
/// **deliberately not account facts** — account-scoped bookkeeping that is **severed**
/// with the account (by explicit child-delete or `ON DELETE CASCADE`) instead of
/// blocking its deletion. Every `accounts`-referencing table must appear in exactly
/// one of this list or [`ACCOUNT_FACT_TABLES`]; the schema tripwire test enforces the
/// classification.
///
/// - `account_members` / `account_invitations` (ZMVP-14/32): membership bookkeeping,
///   deleted children-first by [`hard_delete`](AccountWrites::hard_delete) (their FKs
///   do not cascade). Kept across a *soft*-delete for reactivation, but never a fact
///   that forces one.
/// - `account_handle_changes` (ZMVP-46): the handle-change audit log — `ON DELETE
///   CASCADE`, gone with the account.
/// - `commission_placement` / `commission_current_placement` / `commission_view_grant`
///   (ZMVP-70): the account's **positioning rails** — where a User-owned commission is
///   placed and the revocable view keys held. Each FK onto `accounts` is `ON DELETE
///   CASCADE`, so account hard-delete **severs** them while the commission itself
///   survives untouched (Ownership Separation DD `29130754`; ZMVP-57 AC1). Positioning
///   is environmental — never an account-anchored fact.
pub const ACCOUNT_NON_FACT_TABLES: &[&str] = &[
    "account_members",
    "account_invitations",
    "account_handle_changes",
    "commission_placement",
    "commission_current_placement",
    "commission_view_grant",
];

// Tripwire (ZMVP-57 AC4, mirroring ZMVP-67's commission guard): the constant-`false`
// body of `account_has_facts` (`api/src/routes/accounts.rs`) is sound ONLY while the
// account-fact registry is empty. Registering the first account-fact table makes this
// fail to compile, forcing whoever wires an account-fact store to replace that constant
// with a real EXISTS query over every registered table — and to delete this guard in the
// same, deliberate edit. The `api` crate carries a mirror of this assertion right beside
// the seam it protects.
const _: () = assert!(
    ACCOUNT_FACT_TABLES.is_empty(),
    "ACCOUNT_FACT_TABLES gained an entry: replace the constant-`false` body of \
     account_has_facts in api/src/routes/accounts.rs with a real query over every \
     registered account-fact table (soft-delete an account that bears one), then \
     remove this guard and its mirror in the api crate"
);

/// Rebuild a domain [`Account`] from its generated row. The stored name/handle
/// were validated before they were written, so re-validation here only guards
/// against tampering — surfaced as an error, never a panic.
fn to_account(row: sql::AccountRow) -> anyhow::Result<Account> {
    Ok(Account {
        id: AccountId::new(row.id),
        did: Did::new(row.did),
        handle: Handle::try_new(row.handle)?,
        name: AccountName::try_new(row.name)?,
        created_at: row.created_at,
        updated_at: row.updated_at,
        deleted_at: row.deleted_at,
    })
}

/// Rebuild a domain [`Invitation`] from its generated row, re-validating the
/// stored `role`/`state` discriminants — an `Err` on row tampering, never a panic.
fn to_invitation(row: sql::InvitationRow) -> anyhow::Result<Invitation> {
    Ok(Invitation {
        id: InvitationId::new(row.id),
        account: AccountId::new(row.account_id),
        invited_user: UserId::new(row.invited_user),
        role: Role::try_from(row.role)?,
        inviter: UserId::new(row.inviter),
        state: InvitationState::try_from(row.state)?,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// PostgreSQL read store for accounts and memberships (the [`AccountStore`] read
/// surface). Soft deletes are honored on read ([`find`](PgAccountStore::find)
/// filters `deleted_at IS NULL`). Holds the pool directly — reads pay no
/// transaction tax. The writes live on [`PgAccountWrites`], reached through the
/// [`UnitOfWork`](domain::ports::UnitOfWork). See ZMVP-14 and DESIGN/Account.
pub struct PgAccountStore {
    pool: PgPool,
}

impl PgAccountStore {
    /// Wraps a [`PgPool`] as an [`AccountStore`]. Clones the pool handle (cheap —
    /// it's an `Arc`), so the caller keeps its own.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// PostgreSQL write view over an open transaction (the [`AccountWrites`] surface).
/// Holds **only** a borrowed `&mut PgConnection` — the transaction owned by the
/// [`PgUnitOfWork`](crate::PgUnitOfWork) — so no pool is in scope here and a
/// bare-pool write is unrepresentable. Built by `uow.accounts()`; its borrow ties
/// it to the shared transaction, so writes issued through it commit (or roll back)
/// together with the rest of the unit. See DD `24150017`.
pub struct PgAccountWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    /// Every write executes on `&mut *self.conn`; there is deliberately no pool here.
    pub(crate) conn: &'a mut PgConnection,
}

impl PgAccountWrites<'_> {
    /// Settle a member's departure from `account` on the open transaction — shared
    /// by [`leave`](AccountWrites::leave) and
    /// [`revoke_role`](AccountWrites::revoke_role), which both remove a member with
    /// identical store effects and differ only in the caller's preconditions.
    /// Re-homes the member's children to their parent (DESIGN/Roles rule 3, scoped
    /// to this account), deletes the membership, and revokes the member's
    /// still-pending *issued* invitations so none can later seat a member under a
    /// non-member (DD "Invitation Validity & Issuer Departure" / ZMVP-40 — the
    /// `Revoked` terminal state, never a hard delete). A membership that vanished
    /// under a concurrent removal is a no-op.
    ///
    /// A method on the write view (not a free function taking a connection) so its
    /// writes visibly execute on `self.conn` — the transaction-bound shape the
    /// `no_bare_pool_writes` guard certifies.
    async fn settle_member_departure(
        &mut self,
        user: UserId,
        account: AccountId,
    ) -> anyhow::Result<()> {
        // The member's parent is where their children re-home. If the membership is
        // already gone, there is nothing to settle.
        let Some(parent) = sql::departure_membership(&mut *self.conn, *account, *user).await?
        else {
            return Ok(());
        };

        // Re-home the member's children to the member's parent — scoped to THIS account
        // (`parent` is a `users(id)`, so the same user may be a parent elsewhere).
        sql::departure_rehome_children(&mut *self.conn, parent, *account, *user).await?;

        // The membership itself is removed.
        sql::departure_delete_membership(&mut *self.conn, *account, *user).await?;

        // Revoke the member's still-pending issued invitations.
        sql::departure_revoke_invitations(
            &mut *self.conn,
            InvitationState::Revoked.as_str(),
            Utc::now(),
            *account,
            *user,
            InvitationState::Pending.as_str(),
        )
        .await?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl AccountStore for PgAccountStore {
    /// Filters `deleted_at IS NULL`, so a soft-deleted account reads as `None`
    /// — indistinguishable from one that never existed. The stored `name` is
    /// re-validated through [`AccountName`]; that only guards against row
    /// tampering and surfaces as an `Err`, never a panic.
    async fn find(&self, id: AccountId) -> anyhow::Result<Option<Account>> {
        sql::find(&self.pool, *id)
            .await?
            .map(to_account)
            .transpose()
    }

    async fn role_of(&self, user: UserId, account: AccountId) -> anyhow::Result<Option<Role>> {
        let role = sql::role_of(&self.pool, *user, *account).await?;
        Ok(role.map(Role::try_from).transpose()?)
    }

    /// Selects the lone `state = 'pending'` offer for `(account, invited_user)`, or
    /// `None`. Accepted and revoked invitations are history, not live offers, so
    /// they never match. The stored `role`/`state` discriminants are re-validated
    /// through `Role::try_from`/`InvitationState::try_from` on the way out — an
    /// `Err` on row tampering, never a panic, exactly as `role_of` does for a role.
    async fn find_pending_invitation(
        &self,
        account: AccountId,
        invited_user: UserId,
    ) -> anyhow::Result<Option<Invitation>> {
        sql::find_pending_invitation(
            &self.pool,
            *account,
            *invited_user,
            InvitationState::Pending.as_str(),
        )
        .await?
        .map(to_invitation)
        .transpose()
    }

    /// Loads the invitation for `id` in whatever state it holds (the revoke path
    /// reads it back to weigh authority and current state), or `None`. Stored
    /// discriminants are re-validated on read — an `Err` on tampering, never a panic.
    async fn find_invitation(&self, id: InvitationId) -> anyhow::Result<Option<Invitation>> {
        sql::find_invitation(&self.pool, *id)
            .await?
            .map(to_invitation)
            .transpose()
    }

    /// Exact-match lookup of a live account's `did` by its normalized `handle`,
    /// filtering `deleted_at IS NULL` (a soft-deleted account resolves to `None`,
    /// like [`find`](PgAccountStore::find)). Backs the `/.well-known/atproto-did`
    /// resolver and the founding-time duplicate-handle pre-check. The `UNIQUE`
    /// handle index makes at most one row possible.
    async fn find_did_by_handle(&self, handle: &Handle) -> anyhow::Result<Option<Did>> {
        let did = sql::find_did_by_handle(&self.pool, handle.as_str()).await?;
        Ok(did.map(Did::new))
    }

    /// Counts the account's `account_handle_changes` rows at or after `since` — the
    /// recent-change tally the change handler weighs against the rate limit. Uses the
    /// `(account_id, changed_at)` index; `count(*)` is never null.
    async fn count_handle_changes_since(
        &self,
        account: AccountId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64> {
        Ok(sql::count_handle_changes_since(&self.pool, *account, since).await?)
    }

    /// `EXISTS` a recent vacation of `handle` by an account other than `excluding` —
    /// i.e. the handle is quarantined to someone else (DD `27852802` §4). `excluding`
    /// (the asking account) is threaded as a nullable uuid so it can reclaim its own
    /// vacated handle: `$3 IS NULL OR account_id <> $3`. Uses the
    /// `(old_handle, changed_at)` index.
    async fn handle_reserved_for_other(
        &self,
        handle: &Handle,
        excluding: Option<AccountId>,
        since: DateTimeUtc,
    ) -> anyhow::Result<bool> {
        let excluding = excluding.map(|account| *account);
        Ok(sql::handle_reserved_for_other(&self.pool, handle.as_str(), since, excluding).await?)
    }
}

/// The unique-violation constraint name carried by an sqlx database error, if
/// any — the receiver for mapping `accounts_handle_key` onto [`HandleTaken`].
fn constraint_of(err: &sqlx::Error) -> Option<&str> {
    match err {
        sqlx::Error::Database(db_err) => db_err.constraint(),
        _ => None,
    }
}

#[async_trait::async_trait]
impl AccountWrites for PgAccountWrites<'_> {
    /// Writes the `accounts` row and the founder's `account_members` row on the
    /// open transaction, so a half-founded account can never be observed and both
    /// rows commit with the rest of the unit. Both rows live in the private store,
    /// so this is one unit of work — never a cross-store dual write.
    ///
    /// A **handle** collision — the global `accounts_handle_key` unique index, which
    /// covers live *and* soft-deleted accounts (a tombstone still reserves its
    /// handle, DD `23003138`) — fails with [`HandleTaken`] as the error source, so
    /// the founding handler maps it to a `409` rather than a `500`. This is the
    /// authoritative backstop for the two cases the handler's pre-check can't see: a
    /// handle reserved by a soft-deleted account, and the concurrent-claim race. A
    /// duplicate `id` or `did` (both machine-minted, never user-facing) stays an
    /// opaque store error.
    async fn create(&mut self, account: &Account, owner: &UserAccount) -> anyhow::Result<()> {
        let insert = sql::create_account(
            &mut *self.conn,
            *account.id,
            account.did.as_str(),
            account.handle.as_str(),
            account.name.as_str(),
            account.created_at,
            account.updated_at,
        )
        .await;

        // Map a handle-uniqueness violation to the typed `HandleTaken` so the caller
        // can answer 409; any other database error stays opaque (→ 500).
        if let Err(ref err) = insert
            && constraint_of(err) == Some("accounts_handle_key")
        {
            return Err(anyhow::Error::new(HandleTaken));
        }
        insert?;

        sql::create_owner_membership(
            &mut *self.conn,
            *owner.account_id,
            *owner.user_id,
            owner.role.as_str(),
        )
        .await?;

        Ok(())
    }

    /// Repoints `accounts.handle` to `new` and appends the change to
    /// `account_handle_changes`, atomically on the open transaction (ZMVP-46, DD
    /// `27852802`). The audit row is what later rate-limits changes (§3) and
    /// quarantines the vacated `old` handle (§4), so it must land with the repoint or
    /// not at all. The public half — re-pointing the DID document's `alsoKnownAs` via
    /// [`DidMinter::update_handle`](domain::ports::DidMinter::update_handle) — is a
    /// separate retryable step the caller runs *first*, never in this transaction (DD
    /// §7; no cross-store dual write).
    ///
    /// `old` is an **optimistic-concurrency precondition**, not just an observation: the
    /// `UPDATE` guards `deleted_at IS NULL` (a live account) **and** `handle = old`, so
    /// it applies only if the row *still* holds the handle the caller saw. It must touch
    /// exactly one row, else the whole unit rolls back — a soft-deleted/vanished account,
    /// or one whose handle changed under a concurrent rename, records **no** audit row,
    /// so the log can never capture a stale `old_handle` (which would leave the truly
    /// vacated handle un-quarantined). A collision with the global `accounts_handle_key`
    /// index (a handle held by another account, live **or** tombstoned — DD 23003138)
    /// surfaces as [`HandleTaken`] so the handler answers `409`, mirroring
    /// [`create`](PgAccountWrites::create). Changing to the account's *own* current
    /// handle is a caller-side no-op rejected before this is reached, so it never hits
    /// the index. `at` is the change instant.
    async fn change_handle(
        &mut self,
        account: AccountId,
        old: &Handle,
        new: &Handle,
        at: DateTimeUtc,
    ) -> anyhow::Result<()> {
        let updated =
            sql::change_handle_repoint(&mut *self.conn, new.as_str(), at, *account, old.as_str())
                .await;

        // Map a handle-uniqueness violation to the typed `HandleTaken` (→ 409), exactly
        // as `create` does; any other database error stays opaque (→ 500).
        if let Err(ref err) = updated
            && constraint_of(err) == Some("accounts_handle_key")
        {
            return Err(anyhow::Error::new(HandleTaken));
        }
        // Zero rows means the precondition failed: the account was soft-deleted/removed,
        // or its handle changed under a concurrent rename since the handler loaded it.
        // Fail so the unit rolls back rather than audit a change against a stale `old`.
        if updated? != 1 {
            anyhow::bail!(
                "change_handle: account {} is not a live account still holding the expected \
                 handle; nothing changed (concurrent change or removal)",
                *account
            );
        }

        sql::change_handle_audit(
            &mut *self.conn,
            uuid::Uuid::now_v7(),
            *account,
            old.as_str(),
            new.as_str(),
            at,
        )
        .await?;

        Ok(())
    }

    /// `INSERT ... ON CONFLICT (account_id, user_id) DO UPDATE`: a new member is
    /// seated, an existing one's role is replaced. `parent` defaults to `NULL`
    /// (role hierarchy deferred), matching the founder row from
    /// [`create`](PgAccountWrites::create).
    async fn grant_role(&mut self, member: &UserAccount) -> anyhow::Result<()> {
        // Upsert: granting a role seats a new member or replaces an existing one's
        // role (DESIGN/Roles — a grant is how a user joins). `parent` is left to
        // its default NULL: the role-hierarchy tree is deferred ("dress when The
        // Who closes"), same as the founder row written by `create`.
        sql::grant_role(
            &mut *self.conn,
            *member.account_id,
            *member.user_id,
            member.role.as_str(),
        )
        .await?;
        Ok(())
    }

    /// A revoke is a member-departure event with the same store effects as
    /// [`leave`](AccountWrites::leave) (the caller settles authority first): re-home
    /// children to the member's parent (DESIGN/Roles rule 3), delete the membership,
    /// and revoke the member's pending issued invitations — atomically on the open
    /// transaction. Revoking a non-member is a harmless no-op.
    async fn revoke_role(&mut self, user: UserId, account: AccountId) -> anyhow::Result<()> {
        self.settle_member_departure(user, account).await
    }

    /// Settle a member leaving the account on the open transaction (ZMVP-21): re-home
    /// the leaver's children to the leaver's parent, delete the membership, and revoke
    /// the leaver's still-pending issued invitations. Preconditions (must be a member,
    /// can't be the `Owner`) are the caller's; a membership that vanished under a
    /// concurrent removal is a no-op. See the [`leave`](AccountWrites::leave) port doc.
    async fn leave(&mut self, user: UserId, account: AccountId) -> anyhow::Result<()> {
        self.settle_member_departure(user, account).await
    }

    /// `INSERT ... ON CONFLICT (account_id, invited_user) WHERE state = 'pending'
    /// DO NOTHING`: the partial unique index (see the migration) enforces at most
    /// one pending offer per (account, invited user), so a duplicate issue is
    /// silently dropped rather than becoming a second row — the store-level backstop
    /// for the idempotent re-invite the handler also guards by checking
    /// [`find_pending_invitation`](AccountStore::find_pending_invitation) first.
    async fn create_invitation(&mut self, invitation: &Invitation) -> anyhow::Result<()> {
        sql::create_invitation(
            &mut *self.conn,
            *invitation.id,
            *invitation.account,
            *invitation.invited_user,
            invitation.role.as_str(),
            *invitation.inviter,
            invitation.state.as_str(),
            invitation.created_at,
            invitation.updated_at,
        )
        .await?;
        Ok(())
    }

    /// A guarded `UPDATE ... SET state = 'revoked' WHERE id = $1 AND state =
    /// 'pending'`: only a pending offer flips, and an `UPDATE` matching no row still
    /// succeeds — so revoking an absent or already-terminal invitation is a harmless
    /// no-op, not an error (the handler decides whether that's a 404/409). Mirrors
    /// [`revoke_role`](PgAccountWrites::revoke_role)'s no-op-on-no-match shape.
    async fn revoke_invitation(&mut self, id: InvitationId) -> anyhow::Result<()> {
        sql::revoke_invitation(
            &mut *self.conn,
            InvitationState::Revoked.as_str(),
            Utc::now(),
            *id,
            InvitationState::Pending.as_str(),
        )
        .await?;

        Ok(())
    }

    /// Flips the pending invitation to Accepted and seats the invited User as a
    /// member on the open transaction, so the accepted state and the membership
    /// commit together or not at all — the same unit of work as
    /// [`create`](PgAccountWrites::create), never a cross-store dual write. The
    /// `UPDATE` is guarded on `state = 'pending'`; if it matches no row the offer was
    /// already accepted or revoked (a lost race against the handler's in-memory
    /// `Invitation::accept` guard), so this errors and the caller's transaction rolls
    /// back with no membership minted — honoring "a revoked invitation yields no
    /// membership". The new member's `parent` is the `inviter` (DESIGN/Roles rule 4a)
    /// and `listed_on_profile` records the invitee's opt-in.
    async fn accept_invitation(
        &mut self,
        invitation: Invitation,
        listed_on_profile: bool,
    ) -> anyhow::Result<UserAccount> {
        let accepted = sql::accept_invitation_flip(
            &mut *self.conn,
            InvitationState::Accepted.as_str(),
            Utc::now(),
            *invitation.id,
            InvitationState::Pending.as_str(),
        )
        .await?;

        // The guarded UPDATE is the atomic backstop for the handler's in-memory
        // `Invitation::accept` check: matching no pending row means the offer was
        // accepted or revoked in the meantime. Erroring rolls back the caller's
        // transaction rather than seating a member from a spent invitation.
        if accepted == 0 {
            return Err(anyhow::anyhow!(
                "invitation {} is no longer pending; no membership minted",
                *invitation.id
            ));
        }

        let seated = sql::accept_invitation_seat(
            &mut *self.conn,
            *invitation.account,
            *invitation.invited_user,
            *invitation.inviter,
            invitation.role.as_str(),
            listed_on_profile,
        )
        .await?;

        Ok(UserAccount {
            account_id: AccountId::new(seated.account_id),
            user_id: UserId::new(seated.user_id),
            role: Role::try_from(seated.role)?,
        })
    }

    /// Transfer ownership atomically (DESIGN/Roles rule 8): demote the outgoing
    /// Owner to Admin re-homed under the incoming Owner, and promote the incoming
    /// member to Owner with no parent (rule 5). Both `UPDATE`s ride the one
    /// transaction-bound connection, so they commit together or not at all.
    ///
    /// Each precondition is enforced **inside** its mutating statement — the demotion
    /// only fires while the actor is *still* the Owner (`role = 'owner'`), the
    /// promotion only while the target is *still* a member — and each must touch
    /// exactly one row or the whole unit rolls back. Guarding the mutation itself
    /// (rather than a prior `SELECT`) is what makes the single-Owner invariant
    /// unreachable to violate under concurrency: two simultaneous transfers of the
    /// same account serialize on the Owner row, so the second one's demotion matches
    /// zero rows (the Owner is already demoted) and fails closed — it can never mint a
    /// second Owner. The caller (the handler) still settles authority up front for the
    /// friendly `403`/`404`; these guards are the last line that also survives a race.
    async fn transfer_ownership(
        &mut self,
        old_owner: UserId,
        new_owner: UserId,
        account: AccountId,
    ) -> anyhow::Result<()> {
        // Demote the outgoing Owner to Admin, re-homed under the incoming Owner — but
        // only while they are *still* the Owner. A race that already moved ownership
        // leaves this matching zero rows, so we error and roll back.
        let demoted = sql::transfer_demote_owner(
            &mut *self.conn,
            Role::Admin(None).as_str(),
            *account,
            *old_owner,
            *new_owner,
            Role::Owner(None).as_str(),
        )
        .await?;
        if demoted != 1 {
            anyhow::bail!(
                "transfer_ownership: user {} is not the current Owner of account {}; nothing transferred",
                *old_owner,
                *account
            );
        }

        // Promote the incoming member to sole Owner with no parent — but only while
        // they are *still* a member. Zero rows means they vanished mid-transfer, so we
        // error and roll back rather than leave the account with no Owner.
        let promoted = sql::transfer_promote_heir(
            &mut *self.conn,
            Role::Owner(None).as_str(),
            *account,
            *new_owner,
        )
        .await?;
        if promoted != 1 {
            anyhow::bail!(
                "transfer_ownership: user {} is not a member of account {}; nothing transferred",
                *new_owner,
                *account
            );
        }

        Ok(())
    }

    /// `UPDATE accounts SET deleted_at = now WHERE id = $1 AND deleted_at IS NULL`:
    /// stamps the soft-delete marker (and `updated_at`) on a live account, keeping the
    /// row — so the handle stays reserved (global index) and the DID stays live, while
    /// [`find`](PgAccountStore::find) now reads it as absent. Memberships and
    /// invitations are left untouched (a reactivation restores them). The
    /// `deleted_at IS NULL` guard makes a repeat soft-delete a harmless no-op. See the
    /// [`soft_delete`](AccountWrites::soft_delete) port doc.
    async fn soft_delete(&mut self, account: AccountId) -> anyhow::Result<()> {
        sql::soft_delete(&mut *self.conn, Utc::now(), *account).await?;
        Ok(())
    }

    /// Deletes the account's `account_invitations`, then `account_members`, then the
    /// `accounts` row — those two child FKs do not cascade, so they are removed
    /// children-first. The account's other children **do** cascade on the final
    /// `DELETE accounts`: the `account_handle_changes` audit log (ZMVP-46) and the
    /// positioning rails — `commission_placement`, `commission_current_placement`, and
    /// `commission_view_grant` (ZMVP-70) — each carry `ON DELETE CASCADE` on their FK
    /// onto `accounts`, so they are **severed** with the account while the placed
    /// commissions survive untouched (Ownership Separation DD `29130754`; ZMVP-57 AC1).
    /// Removing the `accounts` row **frees its handle** from the global unique index for
    /// reuse. The custody `account_keys` row is deliberately **not** touched here (the
    /// ~72h PLC recovery window can still reverse the tombstone). All the deletes run on
    /// the open transaction, so an empty account is removed atomically; a `DELETE`
    /// matching no row is a no-op. See the [`hard_delete`](AccountWrites::hard_delete)
    /// port doc.
    async fn hard_delete(&mut self, account: AccountId) -> anyhow::Result<()> {
        sql::hard_delete_invitations(&mut *self.conn, *account).await?;
        sql::hard_delete_memberships(&mut *self.conn, *account).await?;
        sql::hard_delete_account(&mut *self.conn, *account).await?;
        Ok(())
    }
}
