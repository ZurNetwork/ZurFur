//! [`CommissionWrites`] over PostgreSQL: commissions in the `commission` table
//! (ZMVP-65). Writes are reachable only on an open [`UnitOfWork`](domain::ports::UnitOfWork)
//! (`uow.commissions()`), so no commission write can skip a transaction. There is no
//! read store yet — the birth ticket only needs the create path. See DESIGN/Commission
//! and DD `24150017` (compile-enforced Unit of Work).

use domain::{elements::commission::Commission, ports::CommissionWrites};
use sqlx::{PgConnection, query};

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
}
