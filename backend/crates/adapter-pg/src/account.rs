//! [`AccountRepo`] over PostgreSQL: accounts and their memberships in the
//! `accounts` / `account_members` tables. See ZMVP-14 and DESIGN/Account.

use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        did::Did,
        invitation::{Invitation, InvitationId, InvitationState},
        role::Role,
        user::UserId,
        user_account::UserAccount,
    },
    ports::AccountRepo,
};
use sqlx::{PgPool, query};

/// PostgreSQL implementation of [`AccountRepo`] — accounts and their memberships
/// across the `accounts` and `account_members` tables. Soft deletes are honored
/// on read ([`find`](PgAccountRepo::find) filters `deleted_at IS NULL`); the
/// role-hierarchy tree (`parent`) is left for a later ticket. See ZMVP-14 and
/// DESIGN/Account.
pub struct PgAccountRepo {
    pool: PgPool,
}

impl PgAccountRepo {
    /// Wraps a [`PgPool`] as an [`AccountRepo`]. Clones the pool handle (cheap —
    /// it's an `Arc`), so the caller keeps its own.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl AccountRepo for PgAccountRepo {
    /// Writes the `accounts` row and the founder's `account_members` row inside a
    /// single transaction, so a half-founded account can never be observed. Both
    /// rows live in the private store, so this is one unit of work — never a
    /// cross-store dual write. A duplicate `id` or `did` surfaces as the unique
    /// constraint's error.
    async fn create(&self, account: &Account, owner: &UserAccount) -> anyhow::Result<()> {
        let mut tx = self.pool.begin().await?;
        query!(
            r#"
        INSERT INTO accounts (id, did, name, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5)
        "#,
            *account.id,
            account.did.as_str(),
            account.name.as_str(),
            account.created_at,
            account.updated_at
        )
        .execute(&mut *tx)
        .await?;

        query!(
            r#"
        INSERT INTO account_members (account_id, user_id, role)
        VALUES ($1, $2, $3)
        "#,
            *owner.get_account_id(),
            *owner.get_user_id(),
            owner.get_role().as_str()
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(())
    }

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

        // The stored name was validated before it was written, so re-validation
        // here only guards against tampering — surfaced as an error, never a panic.
        row.map(|row| {
            Ok(Account {
                id: AccountId::new(row.id),
                did: Did::new(row.did),
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

    /// `INSERT ... ON CONFLICT (account_id, user_id) DO UPDATE`: a new member is
    /// seated, an existing one's role is replaced. `parent` defaults to `NULL`
    /// (role hierarchy deferred), matching the founder row from
    /// [`create`](PgAccountRepo::create).
    async fn grant_role(&self, member: &UserAccount) -> anyhow::Result<()> {
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
            *member.get_account_id(),
            *member.get_user_id(),
            member.get_role().as_str()
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// A `DELETE` that matches no row still succeeds, so revoking a non-member is
    /// a harmless no-op (the handler decides whether absence is a 404).
    async fn revoke_role(&self, user: UserId, account: AccountId) -> anyhow::Result<()> {
        // Remove the membership — the inverse of `grant_role`. A DELETE that matches
        // no row affects nothing and still succeeds, so revoking a non-member is a
        // harmless no-op (the handler decides whether that's a 404 for the caller).
        query!(
            r#"
        DELETE FROM account_members
        WHERE user_id = $1 AND account_id = $2
        "#,
            *user,
            *account,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// `INSERT ... ON CONFLICT (account_id, invited_user) WHERE state = 'pending'
    /// DO NOTHING`: the partial unique index (see the migration) enforces at most
    /// one pending offer per (account, invited user), so a duplicate issue is
    /// silently dropped rather than becoming a second row — the store-level backstop
    /// for the idempotent re-invite the handler also guards by checking
    /// [`find_pending_invitation`](PgAccountRepo::find_pending_invitation) first.
    async fn create_invitation(&self, invitation: &Invitation) -> anyhow::Result<()> {
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
            .execute(&self.pool)
        .await?;
        Ok(())
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

    /// A guarded `UPDATE ... SET state = 'revoked' WHERE id = $1 AND state =
    /// 'pending'`: only a pending offer flips, and an `UPDATE` matching no row still
    /// succeeds — so revoking an absent or already-terminal invitation is a harmless
    /// no-op, not an error (the handler decides whether that's a 404/409). Mirrors
    /// [`revoke_role`](PgAccountRepo::revoke_role)'s no-op-on-no-match shape.
    async fn revoke_invitation(&self, id: InvitationId) -> anyhow::Result<()> {
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
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
