//! [`CommissionStore`] (reads) and [`CommissionWrites`] (writes) over PostgreSQL:
//! commissions in the `commission` table (ZMVP-65/87). Reads are pool-backed;
//! writes are reachable only on an open [`UnitOfWork`](domain::ports::UnitOfWork)
//! (`uow.commissions()`), so no commission write can skip a transaction. See
//! DESIGN/Commission and DD `24150017` (compile-enforced Unit of Work).

use domain::{
    elements::{
        commission::{
            ChannelPointer, Commission, CommissionId, CommissionTitle, LifecycleStep, Visibility,
        },
        user::UserId,
    },
    ports::{CommissionStore, CommissionWrites},
};
use sqlx::{PgConnection, PgPool, query};

/// THE FACT REGISTRY (ZMVP-67; Deletion DD `3014657`): the tables whose rows are
/// commission [`Fact`](domain::elements::commission::Fact)s — evidence that blocks
/// hard deletion. [`commission_has_facts`](CommissionWrites::commission_has_facts)
/// must query **every** table listed here; the DD's canonical trigger list names
/// the kinds to expect (Products, ratings, EXP, achievements, payments), none of
/// which exist yet.
///
/// Registering a table here is a **deliberate act with teeth**: the schema
/// tripwire test (`adapter-pg/tests/commission.rs`) fails the moment a migration
/// adds a commission-referencing table that is classified in neither this list nor
/// [`COMMISSION_NON_FACT_TABLES`], and the compile-time guard below refuses to
/// build while this list is non-empty but the predicate is still constant `false`.
/// A fact-minter therefore wires its storage into the predicate in the same change
/// that creates it — it cannot merge past either trip by accident.
pub const COMMISSION_FACT_TABLES: &[&str] = &[];

/// Tables that hold a foreign key onto `commission(id)` but whose rows are
/// **deliberately not facts** — commission-owned bookkeeping that cascades away
/// with the commission instead of blocking its deletion. Every
/// commission-referencing table must appear in exactly one of this list or
/// [`COMMISSION_FACT_TABLES`]; the schema tripwire test enforces the
/// classification.
///
/// - `commission_changelog` (ZMVP-87): the commission's own memory. The Changelog
///   DD's retention rule — entries hard-delete **only** with the commission itself
///   (or legal duty) — is exactly `ON DELETE CASCADE`, not a deletion block.
pub const COMMISSION_NON_FACT_TABLES: &[&str] = &["commission_changelog"];

// Tripwire (conductor ruling E18): the constant-`false` body of
// `commission_has_facts` below is sound ONLY while the fact registry is empty.
// Registering the first fact table makes this fail to compile, forcing whoever
// wires a fact-minter to replace the constant with a real EXISTS query over every
// registered table — and to delete this guard in the same, deliberate edit.
const _: () = assert!(
    COMMISSION_FACT_TABLES.is_empty(),
    "COMMISSION_FACT_TABLES gained an entry: replace the constant-`false` body of \
     PgCommissionWrites::commission_has_facts with a real query over every \
     registered fact table (and mirror it in adapter-mem), then remove this guard"
);

/// PostgreSQL write view over an open transaction (the [`CommissionWrites`] surface).
/// Holds **only** a borrowed `&mut PgConnection` — the transaction owned by the
/// [`PgUnitOfWork`](crate::PgUnitOfWork) — so no pool is in scope here and a
/// bare-pool write is unrepresentable. Built by `uow.commissions()`; its borrow ties
/// it to the shared transaction, so its write commits (or rolls back) with the rest
/// of the unit. See DD `24150017`.
pub struct PgCommissionWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    /// The write executes on `&mut *self.conn`; there is deliberately no pool here.
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait::async_trait]
impl CommissionWrites for PgCommissionWrites<'_> {
    /// Insert a freshly created commission as one row (`INSERT INTO commission`).
    /// The [`LifecycleStep`](domain::elements::commission::LifecycleStep) and
    /// [`Visibility`](domain::elements::commission::Visibility) are each stored as their
    /// stable `as_str()` token in the `lifecycle` / `visibility` text columns, and the
    /// nullable deadline maps to a nullable `timestamptz`. The id is a caller-minted
    /// UUIDv7, so no conflict handling is needed; any store failure surfaces as an
    /// opaque error.
    async fn create(&mut self, commission: &Commission) -> anyhow::Result<()> {
        query!(
            r#"
            INSERT INTO
            commission (
                id,
                title,
                owner_id,
                lifecycle,
                visibility,
                deadline,
                created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
            *commission.id,
            commission.title.as_str(),
            *commission.owner_id,
            commission.lifecycle_step.as_str(),
            commission.visibility.as_str(),
            commission.deadline,
            commission.created_at
        )
        .execute(&mut *self.conn)
        .await?;
        Ok(())
    }

    /// Whether the commission bears any fact — answered **on the open transaction**,
    /// so a delete gate's check-then-delete has no TOCTOU window (ZMVP-67, ruling E17).
    ///
    /// Constant `false` today, and sound only by construction: the fact registry
    /// ([`COMMISSION_FACT_TABLES`]) is empty because no fact-minter exists — no
    /// table anywhere holds commission-anchored facts, so no query could find one.
    /// This is **not** a stub to fill casually: the compile-time guard on the
    /// registry refuses to build the moment a table is registered, and the schema
    /// tripwire test refuses any commission-referencing table that skips
    /// classification — so this body becomes a real `EXISTS` over every registered
    /// table in the same change that mints the first fact (Deletion DD `3014657`).
    async fn commission_has_facts(&mut self, _id: CommissionId) -> anyhow::Result<bool> {
        Ok(false)
    }

    /// Remove the commission row — one `DELETE FROM commission` on the open
    /// transaction, so the caller's fact gate
    /// ([`commission_has_facts`](CommissionWrites::commission_has_facts)) and the
    /// delete commit or roll back together (ZMVP-66, ruling E17). Child rows reap
    /// via each commission-referencing table's `ON DELETE CASCADE` (ruling E35;
    /// today `commission_changelog` — see [`COMMISSION_NON_FACT_TABLES`], whose
    /// tripwire keeps every future child classified). An absent commission
    /// matches no row: a no-op, per the port contract.
    async fn delete(&mut self, id: CommissionId) -> anyhow::Result<()> {
        query!(r#"DELETE FROM commission WHERE id = $1"#, *id)
            .execute(&mut *self.conn)
            .await?;
        Ok(())
    }

    /// Repoint (or clear) the `commission.linked_channel` column — one
    /// **conditional** `UPDATE` on the open transaction: the row matches only
    /// when the stored value differs from the requested one
    /// (`IS DISTINCT FROM`, so NULLs compare honestly), making rows-affected THE
    /// changed answer. The caller keys its changelog append on the bool in this
    /// same unit of work (ZMVP-87 AC3; Changelog DD D4), so a duplicate
    /// `channel_linked`/`channel_unlinked` entry is unrepresentable even under
    /// concurrent writers. An absent commission matches no row and answers
    /// `false`, per the port contract (existence is the caller's check).
    async fn set_linked_channel(
        &mut self,
        id: CommissionId,
        channel: Option<&ChannelPointer>,
    ) -> anyhow::Result<bool> {
        let result = query!(
            r#"
            UPDATE commission
            SET linked_channel = $2
            WHERE id = $1 AND linked_channel IS DISTINCT FROM $2
            "#,
            *id,
            channel.map(ChannelPointer::as_str),
        )
        .execute(&mut *self.conn)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

/// PostgreSQL read store for commissions (the [`CommissionStore`] surface) —
/// the one canonical commission read port, born with the changelog (ZMVP-87).
/// Holds the pool directly — reads pay no transaction tax; the writes live on
/// [`PgCommissionWrites`], reached through the [`UnitOfWork`](domain::ports::UnitOfWork).
pub struct PgCommissionStore {
    pool: PgPool,
}

impl PgCommissionStore {
    /// Wraps a [`PgPool`] as a [`CommissionStore`]. Clones the pool handle (cheap —
    /// it's an `Arc`), so the caller keeps its own.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl CommissionStore for PgCommissionStore {
    /// Rebuild the [`Commission`] from its row. The stored `lifecycle`,
    /// `visibility`, and `linked_channel` values are re-validated through their
    /// domain gates ([`LifecycleStep::parse`] / [`Visibility::parse`] /
    /// [`ChannelPointer::try_new`], with [`CommissionTitle::try_new`] for the
    /// title); a value outside its vocabulary means row tampering and surfaces
    /// as an `Err`, never a panic or a silent default.
    async fn find(&self, id: CommissionId) -> anyhow::Result<Option<Commission>> {
        let Some(row) = query!(
            r#"
            SELECT title, owner_id, lifecycle, visibility, deadline, linked_channel, created_at
            FROM commission
            WHERE id = $1
            "#,
            *id,
        )
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };

        Ok(Some(Commission {
            id,
            title: CommissionTitle::try_new(row.title)?,
            owner_id: UserId::new(row.owner_id),
            lifecycle_step: LifecycleStep::parse(&row.lifecycle)
                .ok_or_else(|| anyhow::anyhow!("unknown lifecycle token {:?}", row.lifecycle))?,
            visibility: Visibility::parse(&row.visibility)
                .ok_or_else(|| anyhow::anyhow!("unknown visibility token {:?}", row.visibility))?,
            deadline: row.deadline,
            linked_channel: row
                .linked_channel
                .map(ChannelPointer::try_new)
                .transpose()?,
            created_at: row.created_at,
        }))
    }

    /// The **owner arm** of participant-hood (ZMVP-87): one `EXISTS` over the
    /// owner column — the owner IS a Participant without holding a Seat
    /// (DESIGN/Commission). ZMVP-79 extends this query with the seated arm; an
    /// unknown commission matches nothing and answers `false`.
    async fn is_participant(&self, commission: CommissionId, user: UserId) -> anyhow::Result<bool> {
        let row = query!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM commission WHERE id = $1 AND owner_id = $2
            ) AS "is_participant!"
            "#,
            *commission,
            *user,
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.is_participant)
    }
}
