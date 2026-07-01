//! [`AccountStore`] (reads) and [`AccountWrites`] (writes) over PostgreSQL:
//! accounts and their memberships in the `accounts` / `account_members` tables.
//! Reads are pool-backed; writes are reachable only on an open [`UnitOfWork`]
//! (`uow.accounts()`), so no account write can skip a transaction. See ZMVP-14,
//! DESIGN/Account, and DD `24150017` (compile-enforced Unit of Work).

use chrono::Utc;
use domain::{
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
use sqlx::{PgConnection, PgPool, query};

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

/// Settle a member's departure from `account` on an open transaction — shared by
/// [`leave`](AccountWrites::leave) and [`revoke_role`](AccountWrites::revoke_role),
/// which both remove a member with identical store effects and differ only in the
/// caller's preconditions. Re-homes the member's children to their parent
/// (DESIGN/Roles rule 3, scoped to this account), deletes the membership, and
/// revokes the member's still-pending *issued* invitations so none can later seat a
/// member under a non-member (DD "Auth Surfaces, the Plugin Trust Boundary & CSRF" /
/// ZMVP-40 — the `Revoked` terminal state, never a hard delete). A membership that
/// vanished under a concurrent removal is a no-op.
async fn settle_member_departure(
    conn: &mut PgConnection,
    user: UserId,
    account: AccountId,
) -> anyhow::Result<()> {
    // The member's parent is where their children re-home. If the membership is
    // already gone, there is nothing to settle.
    let Some(membership) = query!(
        r#"SELECT parent FROM account_members WHERE account_id = $1 AND user_id = $2"#,
        *account,
        *user,
    )
    .fetch_optional(&mut *conn)
    .await?
    else {
        return Ok(());
    };

    // Re-home the member's children to the member's parent — scoped to THIS account
    // (`parent` is a `users(id)`, so the same user may be a parent elsewhere).
    query!(
        r#"UPDATE account_members SET parent = $1 WHERE account_id = $2 AND parent = $3"#,
        membership.parent,
        *account,
        *user,
    )
    .execute(&mut *conn)
    .await?;

    // The membership itself is removed.
    query!(
        r#"DELETE FROM account_members WHERE account_id = $1 AND user_id = $2"#,
        *account,
        *user,
    )
    .execute(&mut *conn)
    .await?;

    // Revoke the member's still-pending issued invitations.
    query!(
        r#"UPDATE account_invitations SET state = $1, updated_at = $2
           WHERE account_id = $3 AND inviter = $4 AND state = $5"#,
        InvitationState::Revoked.as_str(),
        Utc::now(),
        *account,
        *user,
        InvitationState::Pending.as_str(),
    )
    .execute(&mut *conn)
    .await?;

    Ok(())
}

#[async_trait::async_trait]
impl AccountStore for PgAccountStore {
    /// Filters `deleted_at IS NULL`, so a soft-deleted account reads as `None`
    /// — indistinguishable from one that never existed. The stored `name` is
    /// re-validated through [`AccountName`]; that only guards against row
    /// tampering and surfaces as an `Err`, never a panic.
    async fn find(&self, id: AccountId) -> anyhow::Result<Option<Account>> {
        let row = query!(
            r#"
        SELECT
            id      AS "id!",
            did     AS "did!",
            handle  AS "handle!",
            name    AS "name!",
            created_at AS "created_at!: chrono::DateTime<chrono::Utc>",
            updated_at AS "updated_at!: chrono::DateTime<chrono::Utc>",
            deleted_at AS "deleted_at?: chrono::DateTime<chrono::Utc>"
            FROM accounts
            WHERE id = $1
            AND deleted_at IS NULL
        "#,
            *id
        )
        .fetch_optional(&self.pool)
        .await?;

        // The stored name/handle were validated before they were written, so
        // re-validation here only guards against tampering — surfaced as an error,
        // never a panic.
        row.map(|row| {
            Ok(Account {
                id: AccountId::new(row.id),
                did: Did::new(row.did),
                handle: Handle::try_new(row.handle)?,
                name: AccountName::try_new(row.name)?,
                created_at: row.created_at,
                updated_at: row.updated_at,
                deleted_at: row.deleted_at,
            })
        })
        .transpose()
    }

    async fn role_of(&self, user: UserId, account: AccountId) -> anyhow::Result<Option<Role>> {
        let row = query!(
            r#"
        SELECT
            role as "role!"
        FROM account_members
        WHERE 
            user_id = $1 
            AND account_id = $2
        LIMIT 1
        "#,
            *user,
            *account,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|m| Role::try_from(m.role)).transpose()?)
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
        let invitation = query!(
            r#"
            SELECT id, account_id, invited_user, role, inviter, state, created_at, updated_at
            FROM account_invitations
            WHERE account_id = $1 AND invited_user = $2 AND state = $3
            LIMIT 1
        "#,
            *account,
            *invited_user,
            InvitationState::Pending.as_str()
        )
        .fetch_optional(&self.pool)
        .await?;

        invitation
            .map(|i| {
                Ok(Invitation {
                    id: InvitationId::new(i.id),
                    account: AccountId::new(i.account_id),
                    invited_user: UserId::new(i.invited_user),
                    role: Role::try_from(i.role)?,
                    inviter: UserId::new(i.inviter),
                    state: InvitationState::try_from(i.state)?,
                    created_at: i.created_at,
                    updated_at: i.updated_at,
                })
            })
            .transpose()
    }

    /// Loads the invitation for `id` in whatever state it holds (the revoke path
    /// reads it back to weigh authority and current state), or `None`. Stored
    /// discriminants are re-validated on read — an `Err` on tampering, never a panic.
    async fn find_invitation(&self, id: InvitationId) -> anyhow::Result<Option<Invitation>> {
        let invitation = query!(
            r#"
        SELECT id, account_id, invited_user, role, inviter, state, created_at, updated_at
        FROM account_invitations
        WHERE id = $1
        LIMIT 1
        "#,
            *id
        )
        .fetch_optional(&self.pool)
        .await?;

        invitation
            .map(|i| {
                Ok(Invitation {
                    id: InvitationId::new(i.id),
                    account: AccountId::new(i.account_id),
                    invited_user: UserId::new(i.invited_user),
                    role: Role::try_from(i.role)?,
                    inviter: UserId::new(i.inviter),
                    state: InvitationState::try_from(i.state)?,
                    created_at: i.created_at,
                    updated_at: i.updated_at,
                })
            })
            .transpose()
    }

    /// Exact-match lookup of a live account's `did` by its normalized `handle`,
    /// filtering `deleted_at IS NULL` (a soft-deleted account resolves to `None`,
    /// like [`find`](PgAccountStore::find)). Backs the `/.well-known/atproto-did`
    /// resolver and the founding-time duplicate-handle pre-check. The `UNIQUE`
    /// handle index makes at most one row possible.
    async fn find_did_by_handle(&self, handle: &Handle) -> anyhow::Result<Option<Did>> {
        let row = query!(
            r#"SELECT did AS "did!" FROM accounts WHERE handle = $1 AND deleted_at IS NULL"#,
            handle.as_str(),
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| Did::new(row.did)))
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
        let insert = query!(
            r#"
        INSERT INTO accounts (id, did, handle, name, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
            *account.id,
            account.did.as_str(),
            account.handle.as_str(),
            account.name.as_str(),
            account.created_at,
            account.updated_at
        )
        .execute(&mut *self.conn)
        .await;

        // Map a handle-uniqueness violation to the typed `HandleTaken` so the caller
        // can answer 409; any other database error stays opaque (→ 500).
        if let Err(sqlx::Error::Database(ref db_err)) = insert
            && db_err.constraint() == Some("accounts_handle_key")
        {
            return Err(anyhow::Error::new(HandleTaken));
        }
        insert?;

        query!(
            r#"
        INSERT INTO account_members (account_id, user_id, role)
        VALUES ($1, $2, $3)
        "#,
            *owner.account_id,
            *owner.user_id,
            owner.role.as_str()
        )
        .execute(&mut *self.conn)
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
        query!(
            r#"
        INSERT INTO account_members (account_id, user_id, role)
        VALUES ($1, $2, $3)
        ON CONFLICT (account_id, user_id) DO UPDATE
            SET role = EXCLUDED.role
        "#,
            *member.account_id,
            *member.user_id,
            member.role.as_str()
        )
        .execute(&mut *self.conn)
        .await?;
        Ok(())
    }

    /// A revoke is a member-departure event with the same store effects as
    /// [`leave`](AccountWrites::leave) (the caller settles authority first): re-home
    /// children to the member's parent (DESIGN/Roles rule 3), delete the membership,
    /// and revoke the member's pending issued invitations — atomically on the open
    /// transaction. Revoking a non-member is a harmless no-op.
    async fn revoke_role(&mut self, user: UserId, account: AccountId) -> anyhow::Result<()> {
        settle_member_departure(self.conn, user, account).await
    }

    /// Settle a member leaving the account on the open transaction (ZMVP-21): re-home
    /// the leaver's children to the leaver's parent, delete the membership, and revoke
    /// the leaver's still-pending issued invitations. Preconditions (must be a member,
    /// can't be the `Owner`) are the caller's; a membership that vanished under a
    /// concurrent removal is a no-op. See the [`leave`](AccountWrites::leave) port doc.
    async fn leave(&mut self, user: UserId, account: AccountId) -> anyhow::Result<()> {
        settle_member_departure(self.conn, user, account).await
    }

    /// `INSERT ... ON CONFLICT (account_id, invited_user) WHERE state = 'pending'
    /// DO NOTHING`: the partial unique index (see the migration) enforces at most
    /// one pending offer per (account, invited user), so a duplicate issue is
    /// silently dropped rather than becoming a second row — the store-level backstop
    /// for the idempotent re-invite the handler also guards by checking
    /// [`find_pending_invitation`](AccountStore::find_pending_invitation) first.
    async fn create_invitation(&mut self, invitation: &Invitation) -> anyhow::Result<()> {
        query!(r#"
            INSERT INTO account_invitations (id, account_id, invited_user, role, inviter, state, created_at, updated_at)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            ON CONFLICT (account_id, invited_user) WHERE state = 'pending'
            DO NOTHING
        "#,
            *invitation.id,
            *invitation.account,
            *invitation.invited_user,
            invitation.role.as_str(),
            *invitation.inviter,
            invitation.state.as_str(),
            invitation.created_at,
            invitation.updated_at
        )
            .execute(&mut *self.conn)
        .await?;
        Ok(())
    }

    /// A guarded `UPDATE ... SET state = 'revoked' WHERE id = $1 AND state =
    /// 'pending'`: only a pending offer flips, and an `UPDATE` matching no row still
    /// succeeds — so revoking an absent or already-terminal invitation is a harmless
    /// no-op, not an error (the handler decides whether that's a 404/409). Mirrors
    /// [`revoke_role`](PgAccountWrites::revoke_role)'s no-op-on-no-match shape.
    async fn revoke_invitation(&mut self, id: InvitationId) -> anyhow::Result<()> {
        let now = Utc::now();
        query!(
            r#"
            UPDATE account_invitations
            SET state = $1, updated_at = $2
            WHERE id = $3 AND state = $4
            "#,
            InvitationState::Revoked.as_str(),
            now,
            *id,
            InvitationState::Pending.as_str(),
        )
        .execute(&mut *self.conn)
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
        let now = Utc::now();

        let accepted = query!(
            r#"
        UPDATE account_invitations
        SET state = $1, updated_at = $2
        WHERE id = $3 AND state = $4
        "#,
            InvitationState::Accepted.as_str(),
            now,
            *invitation.id,
            InvitationState::Pending.as_str(),
        )
        .execute(&mut *self.conn)
        .await?;

        // The guarded UPDATE is the atomic backstop for the handler's in-memory
        // `Invitation::accept` check: matching no pending row means the offer was
        // accepted or revoked in the meantime. Erroring rolls back the caller's
        // transaction rather than seating a member from a spent invitation.
        if accepted.rows_affected() == 0 {
            return Err(anyhow::anyhow!(
                "invitation {} is no longer pending; no membership minted",
                *invitation.id
            ));
        }

        let new_member = query!(
            r#"
                INSERT INTO account_members (account_id, user_id, parent, "role", listed_on_profile)
                VALUES ($1, $2, $3, $4, $5)
                RETURNING account_id, user_id, "role"
        "#,
            *invitation.account,
            *invitation.invited_user,
            *invitation.inviter,
            invitation.role.as_str(),
            listed_on_profile
        )
        .fetch_one(&mut *self.conn)
        .await?;

        Ok(UserAccount {
            account_id: AccountId::new(new_member.account_id),
            user_id: UserId::new(new_member.user_id),
            role: Role::try_from(new_member.role)?,
        })
    }
    /// Transfer ownership atomically (DESIGN/Roles rule 8): demote the outgoing
    /// Owner to Admin re-homed under the incoming Owner, and promote the incoming
    /// member to Owner with no parent (rule 5). Both `UPDATE`s ride the one
    /// transaction-bound connection, so they commit together or not at all.
    ///
    /// The caller (the handler) has already settled authority — the actor is the
    /// current Owner and the target is an existing member — so the two `SELECT`
    /// guards below are a defensive backstop against a concurrent membership change
    /// between the caller's check and this write; if either precondition has since
    /// vanished the guard errors and the whole unit rolls back (a rare race, mapped
    /// to a 500), rather than leaving the account half-transferred.
    async fn transfer_ownership(
        &mut self,
        old_owner: UserId,
        new_owner: UserId,
        account: AccountId,
    ) -> anyhow::Result<()> {
        query!(
            r#"
            SELECT user_id FROM account_members
            WHERE account_id = $1 AND user_id = $2 AND role = $3
        "#,
            *account,
            *old_owner,
            Role::Owner(None).as_str()
        )
        .fetch_one(&mut *self.conn)
        .await?; // backstop: the outgoing Owner must still be the Owner

        query!(
            r#"
            SELECT user_id FROM account_members
            WHERE account_id = $1 AND user_id = $2
        "#,
            *account,
            *new_owner
        )
        .fetch_one(&mut *self.conn)
        .await?; // backstop: the incoming Owner must still be a member

        query!(
            r#"
            UPDATE account_members
            SET "role" = $1, parent = $4
            WHERE user_id = $2 AND account_id = $3
        "#,
            Role::Admin(None).as_str(),
            *old_owner,
            *account,
            *new_owner
        )
        .execute(&mut *self.conn)
        .await?;

        query!(
            r#"
            UPDATE account_members
            SET "role" = $1, parent = NULL
            WHERE user_id = $2 AND account_id = $3
        "#,
            Role::Owner(None).as_str(),
            *new_owner,
            *account
        )
        .execute(&mut *self.conn)
        .await?;

        Ok(())
    }
}
