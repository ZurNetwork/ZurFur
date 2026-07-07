//! The commission changelog over PostgreSQL (ZMVP-87): appends in the
//! `commission_changelog` table via [`PgChangelogWrites`] — reachable only on an
//! open [`UnitOfWork`](domain::ports::UnitOfWork) (`uow.changelog()`), so an entry
//! commits **atomically with the domain write it records** (Changelog DD
//! `30408741` D4) — and the ordered read via the pool-backed [`PgChangelogStore`].
//! The table itself refuses `UPDATE` (a `BEFORE UPDATE` trigger), so append-only
//! holds even past the ports; `DELETE` stays ungoverned for the commission
//! hard-delete cascade and legal-duty redaction.

use domain::{
    elements::{
        commission::{ChangelogEntry, ChangelogEntryKind, CommissionId, NewChangelogEntry},
        user::UserId,
    },
    ports::{ChangelogStore, ChangelogWrites},
};
use sqlx::{PgConnection, PgPool, query};

/// PostgreSQL append view over an open transaction (the [`ChangelogWrites`]
/// surface). Holds **only** a borrowed `&mut PgConnection` — the transaction
/// owned by the [`PgUnitOfWork`](crate::PgUnitOfWork) — so no pool is in scope
/// here and a pool-backed (dual-write) append is unrepresentable. Built by
/// `uow.changelog()`; its borrow ties it to the shared transaction, so the entry
/// lands (or rolls back) with the domain write it records. See DD `24150017`.
pub struct PgChangelogWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    /// The append executes on `&mut *self.conn`; there is deliberately no pool here.
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait::async_trait]
impl ChangelogWrites for PgChangelogWrites<'_> {
    /// Insert one entry (`INSERT INTO commission_changelog`); Postgres assigns
    /// `seq` (the `bigserial` ordering key). The kind is stored as its stable
    /// [`as_str`](ChangelogEntryKind::as_str) token; a `None` actor lands as SQL
    /// `NULL` (a system entry). Any store failure surfaces as an opaque error.
    async fn append(&mut self, entry: &NewChangelogEntry) -> anyhow::Result<()> {
        query!(
            r#"
            INSERT INTO commission_changelog
                (commission_id, kind, actor_id, payload, note, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            *entry.commission_id,
            entry.kind.as_str(),
            entry.actor_id.as_deref().copied(),
            entry.payload,
            entry.note.as_deref(),
            entry.created_at,
        )
        .execute(&mut *self.conn)
        .await?;
        Ok(())
    }
}

/// PostgreSQL read store for the changelog (the [`ChangelogStore`] surface).
/// Holds the pool directly — reads pay no transaction tax; the append lives on
/// [`PgChangelogWrites`], reached through the [`UnitOfWork`](domain::ports::UnitOfWork).
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
    /// The commission's stream in order (`ORDER BY seq` — the explicit ordering
    /// key; `created_at` is carried for display). Each stored `kind` token is
    /// validated back through [`ChangelogEntryKind::parse`]; an unknown token
    /// means row tampering or a missed migration and surfaces as an error, never
    /// a silent skip.
    async fn entries(&self, commission: CommissionId) -> anyhow::Result<Vec<ChangelogEntry>> {
        let rows = query!(
            r#"
            SELECT seq, kind, actor_id, payload, note, created_at
            FROM commission_changelog
            WHERE commission_id = $1
            ORDER BY seq
            "#,
            *commission,
        )
        .fetch_all(&self.pool)
        .await?;

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
                    commission_id: commission,
                    kind,
                    actor_id: row.actor_id.map(UserId::new),
                    payload: row.payload,
                    note: row.note,
                    created_at: row.created_at,
                })
            })
            .collect()
    }
}
