//! [`UserRepo`] over PostgreSQL: Zurfur's record of recognized visitors in the
//! `users` table, keyed by their sovereign `did`. See ZMVP-9 and DESIGN/User.

use domain::{
    elements::{
        did::Did,
        user::{User, UserId},
    },
    ports::UserRepo,
};
use sqlx::{PgPool, query};

/// PostgreSQL implementation of [`UserRepo`]. Recognizes visitors by their `did`
/// (unique) — it never mints a DID, only the internal `UserId`. See DESIGN/User.
pub struct PgUserRepo {
    pool: PgPool,
}

impl PgUserRepo {
    /// Wraps a [`PgPool`] as a [`UserRepo`]. Clones the pool handle (an `Arc`),
    /// leaving the caller's intact.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl UserRepo for PgUserRepo {
    /// `INSERT ... ON CONFLICT (did) DO UPDATE ... RETURNING`: idempotent and
    /// race-safe in one round trip. The candidate id/created_at minted up front
    /// are discarded on a repeat sign-in, when `RETURNING` hands back the
    /// existing row. The unique `did` constraint — not a check-then-insert — is
    /// the arbiter under concurrent first sign-ins.
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

    /// Read-only lookup by the unique `did` — no INSERT, so an unknown DID
    /// resolves to `None` rather than recognizing a new visitor (the no-mint
    /// counterpart to [`provision`](PgUserRepo::provision)).
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        // Read-only lookup by the unique `did` — no INSERT, so an unknown DID
        // resolves to None rather than recognizing a new visitor.
        let row = query!(
            r#"
            SELECT
                id          AS "id!",
                did         AS "did!",
                created_at  AS "created_at!: chrono::DateTime<chrono::Utc>"
            FROM users
            WHERE did = $1
            "#,
            did.as_str(),
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
