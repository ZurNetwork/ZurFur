//! The commission changelog over PostgreSQL: appends via
//! [`PgChangelogWrites`] on an open [`UnitOfWork`](domain::ports::UnitOfWork)
//! (`uow.changelog()`), so an entry commits atomically with the write it
//! records; ordered reads via the pool-backed [`PgChangelogStore`]. The table
//! refuses `UPDATE` (a trigger); `DELETE` stays open for hard-delete cascades.

use domain::{
    elements::{
        commission::{ChangelogEntry, ChangelogEntryKind, CommissionId, NewChangelogEntry},
        did::Did,
        user::UserId,
    },
    ports::{ChangelogStore, ChangelogWrites},
};
use sqlx::{PgConnection, PgPool};

use crate::queries::changelog as sql;

/// PostgreSQL append view over an open transaction (the [`ChangelogWrites`]
/// surface). Holds only a borrowed `&mut PgConnection`, so a pool-backed
/// (dual-write) append is unrepresentable. Built by `uow.changelog()`.
pub struct PgChangelogWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait::async_trait]
impl ChangelogWrites for PgChangelogWrites<'_> {
    /// Inserts one entry; Postgres assigns `seq` (the ordering key). A `None`
    /// actor lands as SQL `NULL` (a system entry).
    async fn append(&mut self, entry: &NewChangelogEntry) -> anyhow::Result<()> {
        sql::append(
            &mut *self.conn,
            *entry.commission_id,
            entry.kind.as_str(),
            entry.actor_id.as_ref().map(|actor| actor.as_str()),
            &entry.payload,
            entry.note.as_deref(),
            entry.created_at,
        )
        .await?;
        Ok(())
    }
}

/// PostgreSQL read store for the changelog (the [`ChangelogStore`] surface).
/// Holds the pool directly; the append lives on [`PgChangelogWrites`].
pub struct PgChangelogStore {
    pool: PgPool,
}

impl PgChangelogStore {
    /// Wraps a [`PgPool`] as a [`ChangelogStore`]. Clones the pool handle (cheap —
    /// it's an `Arc`), so the caller keeps its own.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ChangelogStore for PgChangelogStore {
    /// The commission's stream in order (`seq`). Each stored `kind` token is
    /// re-validated; an unknown token is an error, never a silent skip.
    async fn entries(&self, commission: &CommissionId) -> anyhow::Result<Vec<ChangelogEntry>> {
        let rows = sql::entries(&self.pool, **commission).await?;

        rows.into_iter()
            .map(|row| {
                let kind = ChangelogEntryKind::parse(&row.kind).ok_or_else(|| {
                    anyhow::anyhow!(
                        "commission_changelog seq {} holds unknown kind token {:?}",
                        row.seq,
                        row.kind,
                    )
                })?;
                Ok(ChangelogEntry {
                    seq: row.seq,
                    commission_id: *commission,
                    kind,
                    actor_id: row.actor_id.map(|did| UserId::new(Did::new(did))),
                    payload: row.payload,
                    note: row.note,
                    created_at: row.created_at,
                })
            })
            .collect()
    }
}
