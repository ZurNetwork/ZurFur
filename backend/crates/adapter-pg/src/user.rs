//! [`UserStore`] (reads) and [`UserWrites`] (recognition) over PostgreSQL:
//! recognized visitors in the `users` table, keyed by their sovereign `did`.
//! Reads are pool-backed; recognition (`provision`) is a write, reachable only
//! on an open [`UnitOfWork`](domain::ports::UnitOfWork) (`uow.users()`).
//! `users` is a projection of the actor super-table shared by every actor kind:
//! `provision` interns the DID, then lands the `users` row under it.

use domain::ports::DidBelongsToAnotherActor;
use domain::{
    elements::{
        actor_identity::{ActorKind, ActorState},
        did::Did,
        user::{User, UserId},
    },
    ports::{UserStore, UserWrites},
};
use sqlx::{PgConnection, PgPool};

use crate::queries::actor_identity as actor_sql;
use crate::queries::user as sql;

/// PostgreSQL read store for recognized visitors (the [`UserStore`] read
/// surface). Recognition (the write) lives on [`PgUserWrites`].
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
/// over an open transaction. Holds only a borrowed `&mut PgConnection`, so
/// recognition cannot skip a transaction. Built by `uow.users()`.
pub struct PgUserWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait::async_trait]
impl UserStore for PgUserStore {
    async fn find(&self, id: &UserId) -> anyhow::Result<Option<User>> {
        Ok(sql::find(&self.pool, id.as_str()).await?.map(|row| User {
            id: UserId::new(Did::new(row.id)),
            created_at: row.created_at,
        }))
    }

    /// Read-only lookup by DID — no INSERT, so an unknown DID resolves to
    /// `None` rather than recognizing a new visitor (the no-mint counterpart
    /// to [`UserWrites::provision`]).
    async fn find_by_did(&self, did: &Did) -> anyhow::Result<Option<User>> {
        Ok(sql::find_by_did(&self.pool, did.as_str())
            .await?
            .map(|row| User {
                id: UserId::new(Did::new(row.id)),
                created_at: row.created_at,
            }))
    }
}

#[async_trait::async_trait]
impl UserWrites for PgUserWrites<'_> {
    /// Recognizes a DID as a two-step write in one unit: `intern` the DID into
    /// the actor super-table, then land the `users` projection under it.
    /// Idempotent and race-safe: a repeat sign-in hits the existing row and
    /// its original `created_at` comes back unchanged.
    async fn provision(&mut self, did: &Did) -> anyhow::Result<User> {
        let now = chrono::Utc::now();

        // Race-safe idempotent upsert: a brand-new DID takes the candidate id;
        // a repeat sign-in collides on the unique `did` and RETURNING hands
        // back the existing identity.
        let candidate_id = uuid::Uuid::now_v7();
        let identity = actor_sql::intern(
            &mut *self.conn,
            candidate_id,
            ActorKind::User.as_str(),
            // Always present: provision is a DID-bearing path.
            Some(did.as_str()),
            ActorState::Active.as_str(),
            now,
        )
        .await?;
        // Fails here with the real error rather than a bewildering FK failure at step 2.
        let identity_is_a_user = identity.kind == ActorKind::User.as_str();
        if !identity_is_a_user {
            let conflict = DidBelongsToAnotherActor {
                existing_kind: identity.kind,
            };
            return Err(anyhow::Error::new(conflict));
        }

        // Idempotent: a repeat sign-in hits the existing row and its original created_at.
        let row = sql::provision(&mut *self.conn, did.as_str(), now).await?;

        Ok(User {
            id: UserId::new(Did::new(row.id)),
            created_at: row.created_at,
        })
    }
}
