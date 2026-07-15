//! [`UserStore`] (reads) and [`UserWrites`] (recognition) over PostgreSQL:
//! Zurfur's record of recognized visitors in the `users` table, keyed by their
//! sovereign `did`. Reads are pool-backed; recognition (`provision`) is a write
//! and so is reachable only on an open [`UnitOfWork`](domain::ports::UnitOfWork)
//! (`uow.users()`). See ZMVP-9, DESIGN/User, and DD `24150017`.
//!
//! The SQL lives in `queries/user/`; the typed functions and the [`UsersRow`]
//! shape are generated against the migrated schema (see [`crate::queries`]).
//!
//! [`UsersRow`]: crate::queries::user::UsersRow

use domain::{
    elements::{
        did::Did,
        user::{User, UserId},
    },
    ports::{UserStore, UserWrites},
};
use sqlx::{PgConnection, PgPool};

use crate::queries::user as sql;

/// Rebuild the domain [`User`] from its generated row.
fn to_user(row: sql::UsersRow) -> User {
    User {
        id: UserId::new(row.id),
        did: Did::new(row.did),
        created_at: row.created_at,
    }
}

/// PostgreSQL read store for recognized visitors (the [`UserStore`] read surface).
/// Resolves a User by id or DID off the pool; recognition (the write) lives on
/// [`PgUserWrites`]. See DESIGN/User.
pub struct PgUserStore {
    pool: PgPool,
}

impl PgUserStore {
    /// Wraps a [`PgPool`] as a [`UserStore`]. Clones the pool handle (an `Arc`),
    /// leaving the caller's intact.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// PostgreSQL write view for recognizing visitors (the [`UserWrites`] surface),
/// over an open transaction. Holds only a borrowed `&mut PgConnection` — the
/// transaction owned by the [`PgUnitOfWork`](crate::PgUnitOfWork) — so recognition
/// cannot skip a transaction. Built by `uow.users()`.
pub struct PgUserWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork);
    /// `provision` executes on `&mut *self.conn`. No pool here, by construction.
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait::async_trait]
impl UserStore for PgUserStore {
    async fn find(&self, id: UserId) -> anyhow::Result<Option<User>> {
        Ok(sql::find(&self.pool, *id).await?.map(to_user))
    }

    /// Read-only lookup by the unique `did` — no INSERT, so an unknown DID
    /// resolves to `None` rather than recognizing a new visitor (the no-mint
    /// counterpart to [`UserWrites::provision`]).
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        Ok(sql::find_by_did(&self.pool, did.as_str())
            .await?
            .map(to_user))
    }
}

#[async_trait::async_trait]
impl UserWrites for PgUserWrites<'_> {
    /// `INSERT ... ON CONFLICT (did) DO UPDATE ... RETURNING`: idempotent and
    /// race-safe in one round trip on the open transaction. The candidate
    /// id/created_at minted up front are discarded on a repeat sign-in, when
    /// `RETURNING` hands back the existing row. The unique `did` constraint — not a
    /// check-then-insert — is the arbiter under concurrent first sign-ins.
    async fn provision(&mut self, did: &Did) -> anyhow::Result<User> {
        // Mint a candidate up front. On a repeat sign-in the INSERT collides on
        // the unique `did`, the no-op DO UPDATE lets RETURNING hand back the
        // *existing* row, and this candidate's id/created_at are discarded. One
        // round trip, and race-safe under concurrent first sign-ins — the unique
        // constraint is the arbiter, not a check-then-insert.
        let candidate = User::recognize(did.clone(), chrono::Utc::now());

        let row = sql::provision(
            &mut *self.conn,
            *candidate.id,
            did.as_str(),
            candidate.created_at,
        )
        .await?;

        Ok(to_user(row))
    }
}
