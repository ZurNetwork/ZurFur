//! Durable tower-sessions [`SessionStore`] over PostgreSQL, backing the session
//! cookie with the `tower_sessions.session` table. Sessions are app-owned rows
//! in the private boundary; persisting them lets a signed-in session survive
//! a reload.

use async_trait::async_trait;
use time::OffsetDateTime;
use tower_sessions_core::{
    SessionStore,
    session::{Id, Record},
    session_store::{self, ExpiredDeletion},
};

use crate::PgPool;
use crate::queries::session as sql;

/// Durable tower-sessions store backing the session cookie with the
/// `tower_sessions.session` table. The whole `Record` is serialized
/// (MessagePack) into the `data` column; `id` and `expiry_date` get their own
/// columns so lookups key on id and filter expired rows in SQL.
#[derive(Clone, Debug)]
pub struct PgSessionStore {
    pool: PgPool,
}

impl PgSessionStore {
    /// Wraps a [`PgPool`] as a tower-sessions [`SessionStore`]. Clones the pool
    /// handle (an `Arc`), so the caller keeps its own.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Serializes a session `Record` to MessagePack for the `data` column. A
/// serialization failure is mapped to [`session_store::Error::Encode`].
fn encode(record: &Record) -> session_store::Result<Vec<u8>> {
    rmp_serde::to_vec(record).map_err(|e| session_store::Error::Encode(e.to_string()))
}

/// Inverse of [`encode`]: reads a `data` column back into a `Record`. A malformed
/// or schema-incompatible blob surfaces as [`session_store::Error::Decode`].
fn decode(data: &[u8]) -> session_store::Result<Record> {
    rmp_serde::from_slice(data).map_err(|e| session_store::Error::Decode(e.to_string()))
}

/// Folds an sqlx error into [`session_store::Error::Backend`], the only error
/// shape the tower-sessions traits expose to the rest of the stack.
fn backend(e: sqlx::Error) -> session_store::Error {
    session_store::Error::Backend(e.to_string())
}

#[async_trait]
impl SessionStore for PgSessionStore {
    /// Inserts under the record's id, retrying with a fresh [`Id`] on a
    /// primary-key collision rather than clobbering an unrelated session.
    /// Mutates `record.id` when it loops.
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        loop {
            let data = encode(record)?;
            let inserted = sql::create(
                &self.pool,
                &record.id.to_string(),
                &data,
                record.expiry_date,
            )
            .await
            .map_err(backend)?;

            if inserted == 1 {
                return Ok(());
            }
            record.id = Id::default();
        }
    }

    /// Upserts by id: persists changes to an existing session, or writes one
    /// whose id is already settled. Unlike [`create`](#method.create) it does
    /// not reroll on collision — the id is the key being saved.
    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let data = encode(record)?;
        sql::save(
            &self.pool,
            &record.id.to_string(),
            &data,
            record.expiry_date,
        )
        .await
        .map_err(backend)?;
        Ok(())
    }

    /// Loads a live session, filtering `expiry_date > now()` in SQL so an
    /// expired row reads as `None` even before [`delete_expired`](#method.delete_expired) sweeps it.
    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let data = sql::load(
            &self.pool,
            &session_id.to_string(),
            OffsetDateTime::now_utc(),
        )
        .await
        .map_err(backend)?;

        data.map(|data| decode(&data)).transpose()
    }

    /// Deletes the session by id. An absent id is a harmless no-op.
    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        sql::delete(&self.pool, &session_id.to_string())
            .await
            .map_err(backend)?;
        Ok(())
    }
}

#[async_trait]
impl ExpiredDeletion for PgSessionStore {
    /// Reaps every row whose `expiry_date` has passed. Read-time expiry is
    /// already enforced by [`load`](#method.load); this just reclaims dead rows.
    async fn delete_expired(&self) -> session_store::Result<()> {
        sql::delete_expired(&self.pool, OffsetDateTime::now_utc())
            .await
            .map_err(backend)?;
        Ok(())
    }
}
