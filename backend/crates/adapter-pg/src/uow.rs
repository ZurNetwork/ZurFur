//! The private-store [`Database`] factory and its [`UnitOfWork`] handle over
//! PostgreSQL — "transactions as a capability" made concrete.
//! [`PgDatabase`] holds the pool and vends a [`PgUnitOfWork`], which holds
//! only the `sqlx::Transaction`; per-aggregate write views borrow it, so a
//! bare-pool write is unrepresentable.

use async_trait::async_trait;
use domain::ports::{
    AccountRepo, ActorIdentityWrites, ChangelogWrites, ColumnWrites, CommissionRepo, Database,
    UnitOfWork, UserWrites, WorkflowWrites,
};
use sqlx::{PgPool, Postgres, Transaction};

use crate::PgCommissionWrites;
use crate::account::PgAccountWrites;
use crate::actor_identity::PgActorIdentityWrites;
use crate::commission_changelog::PgChangelogWrites;
use crate::user::PgUserWrites;
use crate::workflow::{PgColumnWrites, PgWorkflowWrites};

/// The PostgreSQL [`Database`] factory: holds the pool and opens one
/// transaction per [`begin`](Database::begin). Serves no writes itself —
/// those live only on the [`PgUnitOfWork`] it vends.
pub struct PgDatabase {
    pool: PgPool,
}

impl PgDatabase {
    /// Wraps a [`PgPool`] as the write factory. Clones the pool handle (an `Arc`),
    /// so the caller keeps its own.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Database for PgDatabase {
    /// Opens one transaction and hands back the owning handle. Dropping the
    /// handle without [`commit`](UnitOfWork::commit) rolls back.
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        let tx = self.pool.begin().await?;
        Ok(Box::new(PgUnitOfWork { tx }))
    }
}

/// One open PostgreSQL transaction, owned by the handler. Holds only the
/// `Transaction` — the write views reached through it are the only path to
/// a private-store write. [`commit`](UnitOfWork::commit) consumes the handle; dropping it rolls back.
pub struct PgUnitOfWork {
    /// The open transaction (boxable — borrows nothing from the pool beyond a pooled connection).
    tx: Transaction<'static, Postgres>,
}

#[async_trait]
impl UnitOfWork for PgUnitOfWork {
    /// The account repo over this transaction — reads and writes on one connection.
    fn accounts(&mut self) -> Box<dyn AccountRepo + '_> {
        Box::new(PgAccountWrites { conn: &mut self.tx })
    }

    /// The commission repo over this transaction: reads (incl. locking
    /// `*_for_update`) and writes on one connection.
    fn commissions(&mut self) -> Box<dyn CommissionRepo + '_> {
        Box::new(PgCommissionWrites { conn: &mut self.tx })
    }

    /// The changelog append surface over this transaction: an entry commits
    /// atomically with the write it records.
    fn changelog(&mut self) -> Box<dyn ChangelogWrites + '_> {
        Box::new(PgChangelogWrites { conn: &mut self.tx })
    }

    /// The user (recognition) write surface over this transaction.
    fn users(&mut self) -> Box<dyn UserWrites + '_> {
        Box::new(PgUserWrites { conn: &mut self.tx })
    }

    /// The actor-super-table write surface over this transaction. No delete —
    /// identity rows are immortal.
    fn actor_identities(&mut self) -> Box<dyn ActorIdentityWrites + '_> {
        Box::new(PgActorIdentityWrites { conn: &mut self.tx })
    }

    /// The workflow write surface over this transaction: a card's move and
    /// the neighbours it displaces land together.
    fn workflows(&mut self) -> Box<dyn WorkflowWrites + '_> {
        Box::new(PgWorkflowWrites { conn: &mut self.tx })
    }

    /// The column write surface over this transaction.
    fn columns(&mut self) -> Box<dyn ColumnWrites + '_> {
        Box::new(PgColumnWrites { conn: &mut self.tx })
    }

    /// Commits the unit, consuming the handle so it can't be reused.
    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        self.tx.commit().await?;
        Ok(())
    }

    /// Aborts the transaction explicitly and deterministically, consuming the handle.
    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        self.tx.rollback().await?;
        Ok(())
    }
}
