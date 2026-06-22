use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        did::Did,
        role::Role,
        user::UserId,
        user_account::UserAccount,
    },
    ports::AccountRepo,
};
use sqlx::{PgPool, query};

pub struct PgAccountRepo {
    pool: PgPool,
}

impl PgAccountRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl AccountRepo for PgAccountRepo {
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
}
