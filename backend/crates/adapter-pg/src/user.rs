use domain::{
    elements::{
        did::Did,
        user::{User, UserId},
    },
    ports::UserRepo,
};
use sqlx::{PgPool, query};

pub struct PgUserRepo {
    pool: PgPool,
}

impl PgUserRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl UserRepo for PgUserRepo {
    async fn provision(&self, did: &Did) -> anyhow::Result<User> {
        // Mint a candidate up front. On a repeat sign-in the INSERT collides on
        // the unique `did`, the no-op DO UPDATE lets RETURNING hand back the
        // *existing* row, and this candidate's id/created_at are discarded. One
        // round trip, and race-safe under concurrent first sign-ins — the unique
        // constraint is the arbiter, not a check-then-insert.
        let candidate = User::recognize(did.clone(), chrono::Utc::now());

        let row = query!(
            r#"
            INSERT INTO users (id, did, created_at)
            VALUES ($1, $2, $3)
            ON CONFLICT (did) DO UPDATE SET did = EXCLUDED.did
            RETURNING
                id          AS "id!",
                did         AS "did!",
                created_at  AS "created_at!: chrono::DateTime<chrono::Utc>"
            "#,
            *candidate.id,
            did.as_str(),
            candidate.created_at,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(User {
            id: UserId::new(row.id),
            did: Did::new(row.did),
            created_at: row.created_at,
        })
    }

    async fn find(&self, id: UserId) -> anyhow::Result<Option<User>> {
        let row = query!(
            r#"
            SELECT
                id          AS "id!",
                did         AS "did!",
                created_at  AS "created_at!: chrono::DateTime<chrono::Utc>"
            FROM users
            WHERE id = $1
            "#,
            *id,
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| User {
            id: UserId::new(row.id),
            did: Did::new(row.did),
            created_at: row.created_at,
        }))
    }
}
