//! The private-store [`Database`] factory and its [`UnitOfWork`] handle over
//! PostgreSQL — "transactions as a capability" made concrete (DD `24150017`).
//!
//! [`PgDatabase`] holds the pool and vends a [`PgUnitOfWork`]; the unit of work
//! holds **only** the `sqlx::Transaction`, and the per-aggregate write views
//! ([`PgAccountWrites`], [`PgUserWrites`]) borrow that one transaction. No pool is
//! in scope at any write site, so a bare-pool write is unrepresentable; the read
//! stores keep the pool and stay non-transactional. (The profile cache is a
//! documented exception — its best-effort fill is pool-backed, not a domain write;
//! see `PgProfileCache` and the `no_bare_pool_writes` guard.)

use async_trait::async_trait;
use domain::ports::{AccountWrites, Database, UnitOfWork, UserWrites};
use sqlx::{PgPool, Postgres, Transaction};

use crate::account::PgAccountWrites;
use crate::user::PgUserWrites;

/// The PostgreSQL [`Database`] factory: holds the pool and opens one transaction
/// per [`begin`](Database::begin). It serves no writes itself — those live solely
/// on the [`PgUnitOfWork`] it vends — which is exactly what makes a transaction a
/// capability you must hold to write (DD `24150017`).
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
    /// Opens one transaction on the pool and hands back the owning handle. The
    /// pool yields a `Transaction<'static>`, so the boxed [`UnitOfWork`] carries no
    /// borrowed lifetime. Dropping the handle without [`commit`](UnitOfWork::commit)
    /// rolls back (sqlx rolls a dropped, uncommitted transaction back).
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        let tx = self.pool.begin().await?;
        Ok(Box::new(PgUnitOfWork { tx }))
    }
}

/// One open PostgreSQL transaction, owned by the handler. Holds **only** the
/// `Transaction` — no pool — so the write views reached through it (`accounts()`,
/// `users()`, `profiles()`) are the only path to a private-store write, and they
/// all share this one transaction. [`commit`](UnitOfWork::commit) consumes the
/// handle; dropping it rolls back.
pub struct PgUnitOfWork {
    /// The open transaction. `'static` because `PgPool::begin` borrows nothing from
    /// the pool beyond a pooled connection it owns, so the handle is freely boxable.
    tx: Transaction<'static, Postgres>,
}

#[async_trait]
impl UnitOfWork for PgUnitOfWork {
    /// A view of the account write surface over this transaction. The reborrow
    /// `&mut *self.tx` hands the view a `&mut PgConnection` into the shared tx; the
    /// returned box's lifetime ties it to that borrow, so it must be dropped (end of
    /// statement) before the next accessor or before `commit`.
    fn accounts(&mut self) -> Box<dyn AccountWrites + '_> {
        Box::new(PgAccountWrites { conn: &mut self.tx })
    }

    /// A view of the user (recognition) write surface over this transaction.
    fn users(&mut self) -> Box<dyn UserWrites + '_> {
        Box::new(PgUserWrites { conn: &mut self.tx })
    }

    /// Commit the unit, consuming the handle so it can't be reused. Every write
    /// issued through the view accessors lands atomically here.
    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        self.tx.commit().await?;
        Ok(())
    }
}
