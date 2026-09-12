//! [`AccountStore`] (reads) and [`AccountWrites`] (writes) over PostgreSQL:
//! accounts and memberships in `accounts` / `account_members`. Reads are
//! pool-backed; writes only via an open [`UnitOfWork`] (`uow.accounts()`).
//!
//! [`UnitOfWork`]: domain::ports::UnitOfWork

use chrono::Utc;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::{Account, AccountId, AccountMembership, AccountName, ListingScope},
        actor_identity::{ActorKind, ActorState},
        did::Did,
        handle::Handle,
        invitation::{Invitation, InvitationId, InvitationState},
        role::{InvalidRoleAlias, Role, RoleAlias},
        user::UserId,
        user_account::UserAccount,
    },
    ports::{AccountReads, AccountStore, AccountWrites, HandleTaken},
};
use sqlx::{PgConnection, PgPool};
use std::str::FromStr;

use crate::queries::account as sql;
use crate::queries::actor_identity as actor_sql;

/// Account-anchored fact tables: tables whose rows would be orphaned
/// by removing an account, so a bearing account is soft- never hard-deleted.
/// Empty today. Every table referencing `accounts` must appear here or in
/// [`ACCOUNT_NON_FACT_TABLES`]; a schema tripwire test enforces it.
pub const ACCOUNT_FACT_TABLES: &[&str] = &[];

/// Tables with a foreign key onto `accounts(id)` that are deliberately NOT
/// account facts — severed with the account instead of blocking its deletion.
/// Every `accounts`-referencing table must appear here or in
/// [`ACCOUNT_FACT_TABLES`]; a schema tripwire test enforces it.
pub const ACCOUNT_NON_FACT_TABLES: &[&str] = &[
    "workflow",
    "account_members",
    "account_invitations",
    "account_handle_changes",
];

// Fails to compile once ACCOUNT_FACT_TABLES gains an entry — forces replacing
// account_has_facts' constant-`false` body (api/src/routes/accounts.rs) with a
// real query first.
const _: () = assert!(
    ACCOUNT_FACT_TABLES.is_empty(),
    "ACCOUNT_FACT_TABLES gained an entry: replace the constant-`false` body of \
     account_has_facts in api/src/routes/accounts.rs with a real query over every \
     registered account-fact table (soft-delete an account that bears one), then \
     remove this guard and its mirror in the api crate"
);

/// Rebuild a domain [`Account`] from its persisted fields — shared by
/// [`to_account`] and [`to_account_membership`]. Re-validates the stored
/// handle/name; an `Err` on tampering, never a panic. The id IS the account's
/// DID — accounts have no separate surrogate key.
fn build_account(fields: AccountFields) -> anyhow::Result<Account> {
    let AccountFields {
        id,
        handle,
        name,
        created_at,
        updated_at,
        deleted_at,
    } = fields;
    Ok(Account {
        id: AccountId::new(Did::new(id)),
        handle: handle.parse::<Handle>()?,
        name: name.parse::<AccountName>()?,
        created_at,
        updated_at,
        deleted_at,
    })
}

/// Persisted account columns, converged from every generated row shape that
/// carries them. Named fields rather than positional to prevent silently
/// transposing the adjacent `String`/`DateTimeUtc` pairs.
struct AccountFields {
    id: String,
    handle: String,
    name: String,
    created_at: DateTimeUtc,
    updated_at: DateTimeUtc,
    deleted_at: Option<DateTimeUtc>,
}

impl From<sql::FindRow> for AccountFields {
    fn from(row: sql::FindRow) -> Self {
        Self {
            id: row.id,
            handle: row.handle,
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }
    }
}

impl From<sql::FindForUpdateRow> for AccountFields {
    fn from(row: sql::FindForUpdateRow) -> Self {
        Self {
            id: row.id,
            handle: row.handle,
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }
    }
}

impl From<sql::ListForUserRow> for AccountFields {
    fn from(row: sql::ListForUserRow) -> Self {
        Self {
            id: row.id,
            handle: row.handle,
            name: row.name,
            created_at: row.created_at,
            updated_at: row.updated_at,
            deleted_at: row.deleted_at,
        }
    }
}

/// Rebuild a domain [`Account`] from its `find` row. See [`build_account`].
fn to_account(row: sql::FindRow) -> anyhow::Result<Account> {
    build_account(row.into())
}

/// Rebuild a domain [`Account`] from the locking `find_for_update` row. See [`build_account`].
fn to_account_locked(row: sql::FindForUpdateRow) -> anyhow::Result<Account> {
    build_account(row.into())
}

/// Rebuild the caller's optional [`RoleAlias`] from its stored column. An
/// `Err` on a tampered (empty/whitespace) stored value, never a panic.
fn to_role_alias(alias: Option<String>) -> Result<Option<RoleAlias>, InvalidRoleAlias> {
    alias.map(RoleAlias::new).transpose()
}

/// Rebuild an [`AccountMembership`] from a listing row: the account via
/// [`build_account`] plus the caller's [`Role`]/[`RoleAlias`]. Generic over
/// the row shape since self-view and public-projection rows are structurally
/// identical. An `Err` on a tampered stored role, never a panic.
fn to_account_membership<Row>(
    row: Row,
    role: String,
    alias: Option<String>,
) -> anyhow::Result<AccountMembership>
where
    AccountFields: From<Row>,
{
    let role = Role::from_str(&role)?;
    let alias = to_role_alias(alias)?;
    let account = build_account(AccountFields::from(row))?;
    Ok(AccountMembership {
        account,
        role,
        alias,
    })
}

/// Rebuild a domain [`Invitation`] from its generated row, re-validating the
/// stored `role`/`state` discriminants — an `Err` on row tampering, never a panic.
fn to_invitation(row: sql::AccountInvitationsRow) -> anyhow::Result<Invitation> {
    Ok(Invitation {
        id: InvitationId::new(row.id),
        account: AccountId::new(Did::new(row.account_id)),
        invited_user: UserId::new(Did::new(row.invited_user)),
        role: Role::from_str(&row.role)?,
        inviter: UserId::new(Did::new(row.inviter)),
        state: InvitationState::try_from(row.state)?,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

/// PostgreSQL read store for accounts and memberships (the [`AccountStore`]
/// read surface). Soft-deleted accounts read as absent
/// ([`find`](PgAccountStore::find) filters `deleted_at IS NULL`). Writes live
/// on [`PgAccountWrites`], reached through the [`UnitOfWork`](domain::ports::UnitOfWork).
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

/// PostgreSQL write view over an open transaction (the [`AccountWrites`]
/// surface). Holds only a borrowed `&mut PgConnection` — no pool in scope, so
/// a bare-pool write is unrepresentable. Built by `uow.accounts()`.
pub struct PgAccountWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    pub(crate) conn: &'a mut PgConnection,
}

impl PgAccountWrites<'_> {
    /// Settle a member's departure from `account` — shared by
    /// [`leave`](AccountWrites::leave) and [`revoke_role`](AccountWrites::revoke_role).
    /// Re-homes the member's children to their parent, deletes the membership,
    /// and revokes their pending issued invitations. No-op if already gone.
    async fn settle_member_departure(
        &mut self,
        user: &UserId,
        account: &AccountId,
    ) -> anyhow::Result<()> {
        // No-op if the membership is already gone.
        let Some(parent) =
            sql::departure_membership(&mut *self.conn, account.as_str(), user.as_str()).await?
        else {
            return Ok(());
        };

        // Scoped to this account; `parent` is a `users(id)`, may be parent elsewhere.
        sql::departure_rehome_children(
            &mut *self.conn,
            parent.as_deref(),
            account.as_str(),
            user.as_str(),
        )
        .await?;

        sql::departure_delete_membership(&mut *self.conn, account.as_str(), user.as_str()).await?;

        sql::departure_revoke_invitations(
            &mut *self.conn,
            InvitationState::Revoked.as_str(),
            Utc::now(),
            account.as_str(),
            user.as_str(),
            InvitationState::Pending.as_str(),
        )
        .await?;

        Ok(())
    }
}

/// The read half of an account unit of work: the same lookups as
/// [`AccountStore`], executed on the unit's own connection so they see its
/// uncommitted writes. Vended as an [`AccountRepo`](domain::ports::AccountRepo)
/// by `uow.accounts()`.
#[async_trait::async_trait]
impl AccountReads for PgAccountWrites<'_> {
    /// [`AccountStore::find`] on the unit's connection.
    async fn find(&mut self, id: &AccountId) -> anyhow::Result<Option<Account>> {
        sql::find(&mut *self.conn, id.as_str())
            .await?
            .map(to_account)
            .transpose()
    }

    /// [`find`](Self::find) with `FOR NO KEY UPDATE`: concurrent writers wait
    /// for commit; inserts of child rows stay free.
    async fn find_for_update(&mut self, id: &AccountId) -> anyhow::Result<Option<Account>> {
        sql::find_for_update(&mut *self.conn, id.as_str())
            .await?
            .map(to_account_locked)
            .transpose()
    }

    /// [`AccountStore::role_of`] on the unit's connection.
    async fn role_of(
        &mut self,
        user: &UserId,
        account: &AccountId,
    ) -> anyhow::Result<Option<Role>> {
        let role = sql::role_of(&mut *self.conn, user.as_str(), account.as_str()).await?;
        Ok(role.map(|role| Role::from_str(&role)).transpose()?)
    }
}

#[async_trait::async_trait]
impl AccountStore for PgAccountStore {
    /// Filters `deleted_at IS NULL`; a soft-deleted account reads as `None`.
    /// Re-validates the stored `name`; an `Err` on tampering, never a panic.
    async fn find(&self, id: &AccountId) -> anyhow::Result<Option<Account>> {
        sql::find(&self.pool, id.as_str())
            .await?
            .map(to_account)
            .transpose()
    }

    async fn role_of(&self, user: &UserId, account: &AccountId) -> anyhow::Result<Option<Role>> {
        let role = sql::role_of(&self.pool, user.as_str(), account.as_str()).await?;
        Ok(role.map(|role| Role::from_str(&role)).transpose()?)
    }

    /// The lone `state = 'pending'` offer for `(account, invited_user)`, or
    /// `None` — accepted/revoked invitations never match. Re-validates the
    /// stored discriminants; an `Err` on tampering, never a panic.
    async fn find_pending_invitation(
        &self,
        account: &AccountId,
        invited_user: &UserId,
    ) -> anyhow::Result<Option<Invitation>> {
        sql::find_pending_invitation(
            &self.pool,
            account.as_str(),
            invited_user.as_str(),
            InvitationState::Pending.as_str(),
        )
        .await?
        .map(to_invitation)
        .transpose()
    }

    /// The invitation for `id` in whatever state it holds, or `None`.
    /// Re-validates stored discriminants; an `Err` on tampering, never a panic.
    async fn find_invitation(&self, id: &InvitationId) -> anyhow::Result<Option<Invitation>> {
        sql::find_invitation(&self.pool, **id)
            .await?
            .map(to_invitation)
            .transpose()
    }

    /// A live account's `did` by normalized `handle`; filters `deleted_at IS
    /// NULL` like [`find`](PgAccountStore::find). Backs `/.well-known/atproto-did`
    /// resolution and the duplicate-handle pre-check.
    async fn find_did_by_handle(&self, handle: &Handle) -> anyhow::Result<Option<Did>> {
        Ok(sql::find_did_by_handle(&self.pool, handle.as_str())
            .await?
            .map(Did::new))
    }

    /// Count of the account's `account_handle_changes` rows at or after `since`
    /// — the rate-limit tally. Uses the `(account_id, changed_at)` index.
    async fn count_handle_changes_since(
        &self,
        account: &AccountId,
        since: DateTimeUtc,
    ) -> anyhow::Result<i64> {
        Ok(sql::count_handle_changes_since(&self.pool, account.as_str(), since).await?)
    }

    /// Whether `handle` was recently vacated by an account other than
    /// `excluding` — i.e. quarantined to someone else.
    /// Uses the `(old_handle, changed_at)` index.
    async fn handle_reserved_for_other(
        &self,
        handle: &Handle,
        excluding: Option<&AccountId>,
        since: DateTimeUtc,
    ) -> anyhow::Result<bool> {
        let excluding = excluding.map(|account| account.as_str());
        Ok(sql::handle_reserved_for_other(&self.pool, handle.as_str(), since, excluding).await?)
    }

    /// Live memberships for `user`, joined to their accounts and ordered by
    /// id; each row re-validated. `scope` controls `listed_on_profile`:
    /// [`PublicProfile`](ListingScope::PublicProfile) sees only published
    /// memberships, [`SelfView`](ListingScope::SelfView) sees all.
    async fn list_for_user(
        &self,
        user: &UserId,
        scope: ListingScope,
    ) -> anyhow::Result<Vec<AccountMembership>> {
        let honor_privacy = matches!(scope, ListingScope::PublicProfile);
        sql::list_for_user(&self.pool, user.as_str(), honor_privacy)
            .await?
            .into_iter()
            .map(|row| {
                let role = row.role.clone();
                let alias = row.alias.clone();
                to_account_membership(row, role, alias)
            })
            .collect()
    }
}

/// The unique-violation constraint name on an sqlx database error, if any.
fn constraint_of(err: &sqlx::Error) -> Option<&str> {
    match err {
        sqlx::Error::Database(db_err) => db_err.constraint(),
        _ => None,
    }
}

#[async_trait::async_trait]
impl AccountWrites for PgAccountWrites<'_> {
    /// Founds the account: interns its DID into the actor super-table, then writes
    /// the `accounts` row and the founder's `account_members` row, all on the open
    /// transaction. A handle collision on `accounts_handle_key` returns
    /// [`HandleTaken`]; any other failure rolls the transaction back.
    async fn create(&mut self, account: &Account, owner: &UserAccount) -> anyhow::Result<()> {
        // Intern the DID first: the accounts row's composite FK requires the
        // identity row to exist already.
        let interned = actor_sql::intern(
            &mut *self.conn,
            uuid::Uuid::now_v7(),
            ActorKind::Account.as_str(),
            Some(account.id.as_str()),
            ActorState::Active.as_str(),
            account.created_at,
        )
        .await?;
        anyhow::ensure!(
            interned.kind == ActorKind::Account.as_str(),
            "account DID {} is already interned as a different actor kind ({})",
            account.id.as_str(),
            interned.kind
        );

        // Map a handle-uniqueness violation to HandleTaken (409); else opaque (500).
        let insert = sql::create_account(
            &mut *self.conn,
            account.id.as_str(),
            account.handle.as_str(),
            account.name.as_str(),
            account.created_at,
            account.updated_at,
        )
        .await;

        if let Err(ref err) = insert
            && constraint_of(err) == Some("accounts_handle_key")
        {
            return Err(anyhow::Error::new(HandleTaken));
        }
        insert?;

        sql::create_owner_membership(
            &mut *self.conn,
            owner.account_id.as_str(),
            owner.user_id.as_str(),
            owner.role.as_str(),
        )
        .await?;

        Ok(())
    }

    /// Repoints `accounts.handle` to `new` and appends the audit row, atomically.
    /// `old` is an optimistic-concurrency precondition (`handle = old AND
    /// deleted_at IS NULL`); a non-matching row rolls the unit back. A handle
    /// collision returns [`HandleTaken`].
    async fn change_handle(
        &mut self,
        account: &AccountId,
        old: &Handle,
        new: &Handle,
        at: DateTimeUtc,
    ) -> anyhow::Result<()> {
        let updated = sql::change_handle_repoint(
            &mut *self.conn,
            new.as_str(),
            at,
            account.as_str(),
            old.as_str(),
        )
        .await;

        // HandleTaken (409) on a uniqueness violation; else opaque (500).
        if let Err(ref err) = updated
            && constraint_of(err) == Some("accounts_handle_key")
        {
            return Err(anyhow::Error::new(HandleTaken));
        }
        // Zero rows means the precondition failed; roll back rather than audit a stale `old`.
        if updated? != 1 {
            anyhow::bail!(
                "change_handle: account {} is not a live account still holding the expected \
                 handle; nothing changed (concurrent change or removal)",
                account.as_str()
            );
        }

        sql::change_handle_audit(
            &mut *self.conn,
            uuid::Uuid::now_v7(),
            account.as_str(),
            old.as_str(),
            new.as_str(),
            at,
        )
        .await?;

        Ok(())
    }

    /// Upserts the member's role; `parent` defaults to `NULL`, matching the
    /// founder row from [`create`](PgAccountWrites::create).
    async fn grant_role(&mut self, member: &UserAccount) -> anyhow::Result<()> {
        sql::grant_role(
            &mut *self.conn,
            member.account_id.as_str(),
            member.user_id.as_str(),
            member.role.as_str(),
        )
        .await?;
        Ok(())
    }

    /// Departure with the same store effects as [`leave`](AccountWrites::leave)
    /// (the caller settles authority first). No-op on a non-member.
    async fn revoke_role(&mut self, user: &UserId, account: &AccountId) -> anyhow::Result<()> {
        self.settle_member_departure(user, account).await
    }

    /// Settles a member leaving: re-homes children, deletes the membership,
    /// revokes pending issued invitations. Preconditions are the caller's.
    async fn leave(&mut self, user: &UserId, account: &AccountId) -> anyhow::Result<()> {
        self.settle_member_departure(user, account).await
    }

    /// Inserts the invitation, or — a partial unique index enforces at most one
    /// pending offer per (account, invited user) — silently drops a duplicate.
    /// Returns whichever offer now stands: the fresh insert, or the pending one
    /// already on file.
    async fn create_invitation(&mut self, invitation: &Invitation) -> anyhow::Result<Invitation> {
        sql::create_invitation(
            &mut *self.conn,
            *invitation.id,
            invitation.account.as_str(),
            invitation.invited_user.as_str(),
            invitation.role.as_str(),
            invitation.inviter.as_str(),
            invitation.state.as_str(),
            invitation.created_at,
            invitation.updated_at,
        )
        .await?;

        let standing = sql::find_pending_invitation(
            &mut *self.conn,
            invitation.account.as_str(),
            invitation.invited_user.as_str(),
            InvitationState::Pending.as_str(),
        )
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "invitation for account {} user {} is not pending immediately after issuing it",
                invitation.account.as_str(),
                invitation.invited_user.as_str()
            )
        })?;
        to_invitation(standing)
    }

    /// Flips a pending offer to revoked; a no-op (not an error) on an absent or
    /// already-terminal invitation.
    async fn revoke_invitation(&mut self, id: &InvitationId) -> anyhow::Result<()> {
        sql::revoke_invitation(
            &mut *self.conn,
            InvitationState::Revoked.as_str(),
            Utc::now(),
            **id,
            InvitationState::Pending.as_str(),
        )
        .await?;

        Ok(())
    }

    /// Flips the invitation to Accepted and seats the member atomically. Errors
    /// (rolling back) if the offer is no longer pending. Seating is `ON CONFLICT
    /// DO NOTHING`: an already-seated pair is a no-op and the existing role is
    /// re-read rather than overwritten.
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

        // No matching pending row means the offer was already spent; roll back.
        if accepted == 0 {
            return Err(anyhow::anyhow!(
                "invitation {} is no longer pending; no membership minted",
                *invitation.id
            ));
        }

        let seated = sql::accept_invitation_seat(
            &mut *self.conn,
            invitation.account.as_str(),
            invitation.invited_user.as_str(),
            Some(invitation.inviter.as_str()),
            invitation.role.as_str(),
            listed_on_profile,
        )
        .await?;

        // Already seated: RETURNING gave nothing, so read the persisted role instead.
        let (role, alias) = match seated {
            Some(row) => (Role::from_str(&row.role)?, to_role_alias(row.alias)?),
            None => {
                let existing = sql::role_of(
                    &mut *self.conn,
                    invitation.invited_user.as_str(),
                    invitation.account.as_str(),
                )
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "account_members row for account {} user {} vanished between the \
                         conflicting seat and the fallback read",
                        invitation.account.as_str(),
                        invitation.invited_user.as_str()
                    )
                })?;
                (Role::from_str(&existing)?, None)
            }
        };

        Ok(UserAccount {
            account_id: invitation.account,
            user_id: invitation.invited_user,
            role,
            alias,
        })
    }

    /// Transfers ownership atomically: demotes the outgoing Owner to Admin
    /// re-homed under the incoming Owner, promotes the incoming member to Owner.
    /// Each precondition is enforced inside its `UPDATE` and must touch exactly
    /// one row, so a concurrent transfer fails closed rather than minting two Owners.
    async fn transfer_ownership(
        &mut self,
        old_owner: &UserId,
        new_owner: &UserId,
        account: &AccountId,
    ) -> anyhow::Result<()> {
        // Only while old_owner is still Owner; zero rows means a race already moved it.
        let demoted = sql::transfer_demote_owner(
            &mut *self.conn,
            Role::Admin.as_str(),
            account.as_str(),
            old_owner.as_str(),
            Some(new_owner.as_str()),
            Role::Owner.as_str(),
        )
        .await?;
        if demoted != 1 {
            anyhow::bail!(
                "transfer_ownership: user {} is not the current Owner of account {}; nothing transferred",
                old_owner.as_str(),
                account.as_str()
            );
        }

        // Only while new_owner is still a member; zero rows means they vanished mid-transfer.
        let promoted = sql::transfer_promote_heir(
            &mut *self.conn,
            Role::Owner.as_str(),
            account.as_str(),
            new_owner.as_str(),
        )
        .await?;
        if promoted != 1 {
            anyhow::bail!(
                "transfer_ownership: user {} is not a member of account {}; nothing transferred",
                new_owner.as_str(),
                account.as_str()
            );
        }

        Ok(())
    }

    /// Stamps `deleted_at` on a live account, keeping the row (handle stays
    /// reserved, DID stays live); [`find`](PgAccountStore::find) now reads it as
    /// absent. A repeat soft-delete is a no-op.
    async fn soft_delete(&mut self, account: &AccountId) -> anyhow::Result<()> {
        let now = Utc::now();
        sql::soft_delete(&mut *self.conn, Some(now), now, account.as_str()).await?;
        Ok(())
    }

    /// Deletes invitations, then memberships, then the `accounts` row (those FKs
    /// don't cascade). Frees the handle for reuse. Custody `account_keys` rows are
    /// deliberately left in place (the PLC recovery window). A `DELETE` matching
    /// no row is a no-op.
    async fn hard_delete(&mut self, account: &AccountId) -> anyhow::Result<()> {
        sql::hard_delete_invitations(&mut *self.conn, account.as_str()).await?;
        sql::hard_delete_memberships(&mut *self.conn, account.as_str()).await?;
        sql::hard_delete_account(&mut *self.conn, account.as_str()).await?;
        Ok(())
    }
}
