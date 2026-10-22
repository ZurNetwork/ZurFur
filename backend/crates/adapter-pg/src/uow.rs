//! The private-store [`Database`] factory and its [`UnitOfWork`] handle over
//! PostgreSQL — "transactions as a capability" made concrete.
//! [`PgDatabase`] holds the pool and vends a [`PgUnitOfWork`], which holds
//! the `sqlx::Transaction` and its node failure memory; per-aggregate write
//! views borrow it, so a bare-pool write is unrepresentable.

use async_trait::async_trait;
use domain::ports::character::CharacterWrites;
use domain::ports::{
    AccountRepo, ActorIdentityWrites, BugReportWrites, ChangelogWrites, ColumnWrites,
    CommissionRepo, Database, NodeRepo, UnitOfWork, UserWrites, WorkflowWrites,
};
use sqlx::{PgPool, Postgres, Transaction};

use crate::PgCommissionWrites;
use crate::account::PgAccountWrites;
use crate::actor_identity::PgActorIdentityWrites;
use crate::bug_report::PgBugReportWrites;
use crate::character::PgCharacterWrites;
use crate::commission_changelog::PgChangelogWrites;
use crate::node::PgNodeWrites;
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
    #[tracing::instrument(level = "debug", skip_all)]
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        let tx = self.pool.begin().await?;
        Ok(Box::new(PgUnitOfWork { tx, failed: false }))
    }
}

/// One open PostgreSQL transaction, owned by the handler. Holds the
/// `Transaction` and whether a node statement failed on it — the write views
/// reached through it are the only path to a private-store write.
/// [`commit`](UnitOfWork::commit) consumes the handle; dropping it rolls back.
pub struct PgUnitOfWork {
    /// The open transaction (boxable — borrows nothing from the pool beyond a pooled connection).
    tx: Transaction<'static, Postgres>,
    /// Set once a node statement fails. PostgreSQL answers COMMIT in a failed
    /// transaction with a rollback and no error, and sqlx passes that on as success.
    failed: bool,
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

    /// The character write surface over this transaction. Stubbed — no
    /// storage lands until the Character slice.
    fn characters(&mut self) -> Box<dyn CharacterWrites + '_> {
        Box::new(PgCharacterWrites)
    }

    /// The bug-report write surface over this transaction.
    fn bug_reports(&mut self) -> Box<dyn BugReportWrites + '_> {
        Box::new(PgBugReportWrites { conn: &mut self.tx })
    }

    /// The node repo over this transaction; a failed node statement ends the unit.
    fn nodes(&mut self) -> Box<dyn NodeRepo + '_> {
        Box::new(PgNodeWrites {
            conn: &mut self.tx,
            failed: &mut self.failed,
        })
    }

    /// Commits the unit, consuming the handle so it can't be reused. A unit a node
    /// statement ended rolls back instead and returns an error.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        if self.failed {
            self.tx.rollback().await?;
            anyhow::bail!("a refused node statement ended this unit of work; nothing was kept");
        }
        self.tx.commit().await?;
        Ok(())
    }

    /// Aborts the transaction explicitly and deterministically, consuming the handle.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        self.tx.rollback().await?;
        Ok(())
    }
}
