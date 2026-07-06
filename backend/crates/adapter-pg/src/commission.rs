//! [`CommissionWrites`] over PostgreSQL: commissions in the `commission` table
//! (ZMVP-65). Writes are reachable only on an open [`UnitOfWork`](domain::ports::UnitOfWork)
//! (`uow.commissions()`), so no commission write can skip a transaction. There is no
//! read store yet — the birth ticket only needs the create path. See DESIGN/Commission
//! and DD `24150017` (compile-enforced Unit of Work).

use domain::{
    elements::commission::{Commission, CommissionId},
    ports::CommissionWrites,
};
use sqlx::{PgConnection, query};

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
/// with the commission instead of blocking its deletion (a future changelog is the
/// expected first entry). Every commission-referencing table must appear in
/// exactly one of this list or [`COMMISSION_FACT_TABLES`]; the schema tripwire
/// test enforces the classification.
pub const COMMISSION_NON_FACT_TABLES: &[&str] = &[];

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
}
