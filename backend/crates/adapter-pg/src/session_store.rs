use async_trait::async_trait;
use time::OffsetDateTime;
use tower_sessions_core::{
    SessionStore,
    session::{Id, Record},
    session_store::{self, ExpiredDeletion},
};

use crate::PgPool;

/// Durable tower-sessions store backing the session cookie with the
/// `tower_sessions.session` table from this crate's migration. It lives in the
/// private data boundary because sessions are app-owned rows; persisting them
/// here is what lets a signed-in session survive a reload (ZMVP-8).
///
/// The whole `Record` is serialized (MessagePack) into the `data` column, while
/// `id` and `expiry_date` are stored as their own columns so lookups key on the
/// id and filter expired rows in SQL.
#[derive(Clone, Debug)]
pub struct PgSessionStore {
    pool: PgPool,
}

impl PgSessionStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn encode(record: &Record) -> session_store::Result<Vec<u8>> {
    rmp_serde::to_vec(record).map_err(|e| session_store::Error::Encode(e.to_string()))
}

fn decode(data: &[u8]) -> session_store::Result<Record> {
    rmp_serde::from_slice(data).map_err(|e| session_store::Error::Decode(e.to_string()))
}

fn backend(e: sqlx::Error) -> session_store::Error {
    session_store::Error::Backend(e.to_string())
}

#[async_trait]
impl SessionStore for PgSessionStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        // Insert under a fresh id, regenerating on the (vanishingly rare) primary
        // key collision rather than overwriting an unrelated session.
        loop {
            let data = encode(record)?;
            let inserted = sqlx::query(
                "INSERT INTO tower_sessions.session (id, data, expiry_date)
                 VALUES ($1, $2, $3)
                 ON CONFLICT (id) DO NOTHING",
            )
            .bind(record.id.to_string())
            .bind(data)
            .bind(record.expiry_date)
            .execute(&self.pool)
            .await
            .map_err(backend)?;

            if inserted.rows_affected() == 1 {
                return Ok(());
            }
            record.id = Id::default();
        }
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let data = encode(record)?;
        sqlx::query(
            "INSERT INTO tower_sessions.session (id, data, expiry_date)
             VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE
             SET data = excluded.data, expiry_date = excluded.expiry_date",
        )
        .bind(record.id.to_string())
        .bind(data)
        .bind(record.expiry_date)
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let row: Option<(Vec<u8>,)> = sqlx::query_as(
            "SELECT data FROM tower_sessions.session
             WHERE id = $1 AND expiry_date > $2",
        )
        .bind(session_id.to_string())
        .bind(OffsetDateTime::now_utc())
        .fetch_optional(&self.pool)
        .await
        .map_err(backend)?;

        row.map(|(data,)| decode(&data)).transpose()
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        sqlx::query("DELETE FROM tower_sessions.session WHERE id = $1")
            .bind(session_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        Ok(())
    }
}

#[async_trait]
impl ExpiredDeletion for PgSessionStore {
    async fn delete_expired(&self) -> session_store::Result<()> {
        sqlx::query("DELETE FROM tower_sessions.session WHERE expiry_date < $1")
            .bind(OffsetDateTime::now_utc())
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        Ok(())
    }
}
