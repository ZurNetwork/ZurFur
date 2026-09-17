use std::future::Future;

use async_trait::async_trait;

use super::AccountRepo;
use super::UserWrites;
use super::actor_identity::ActorIdentityWrites;
use super::changelog::ChangelogWrites;
use super::character::CharacterWrites;
use super::commission::CommissionRepo;
use super::workflow::{ColumnWrites, WorkflowWrites};

/// The factory for a private-store [`UnitOfWork`] — the only way to reach a
/// private-store write, which is therefore unrepresentable without first opening
/// a transaction. Aggregate-neutral: one `begin()` opens one transaction for
/// writes across any number of aggregates.
#[async_trait]
pub trait Database: Send + Sync {
    /// Begin one private-store transaction; the returned handle owns it.
    /// Intra-Postgres only — never a cross-store dual write.
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>>;
}

/// One open private-store transaction. Aggregate writes are reached as views over
/// it through the accessors (`uow.accounts().create(…)`), so every write in the
/// unit lands on [`commit`](UnitOfWork::commit) or not at all. The handle holds
/// no pool, so nothing on this path can skip the transaction.
#[async_trait]
pub trait UnitOfWork: Send {
    /// The [`Account`] repo over this transaction ([`AccountRepo`]). The box
    /// borrows the handle; drop it before calling another accessor.
    fn accounts(&mut self) -> Box<dyn AccountRepo + '_>;

    /// The commission repo over this transaction, reads and writes
    /// ([`CommissionRepo`]).
    fn commissions(&mut self) -> Box<dyn CommissionRepo + '_>;

    /// The commission-changelog append surface over this transaction, so an entry
    /// commits atomically with the domain write it records.
    fn changelog(&mut self) -> Box<dyn ChangelogWrites + '_>;

    /// The [`User`] write surface (recognition) over this transaction.
    fn users(&mut self) -> Box<dyn UserWrites + '_>;

    /// The actor-super-table write surface over this transaction. It carries no
    /// delete — identity rows are immortal.
    fn actor_identities(&mut self) -> Box<dyn ActorIdentityWrites + '_>;

    /// The workflow write surface over this transaction; a board mutation is
    /// rarely one row.
    fn workflows(&mut self) -> Box<dyn WorkflowWrites + '_>;

    /// The column write surface over this transaction. Column *order* is a
    /// workflow write, not a column one.
    fn columns(&mut self) -> Box<dyn ColumnWrites + '_>;

    fn characters(&mut self) -> Box<dyn CharacterWrites + '_>;

    /// Commit the unit, consuming the handle; every write lands atomically.
    /// Dropping the handle instead rolls the whole unit back.
    async fn commit(self: Box<Self>) -> anyhow::Result<()>;

    /// Abort the unit explicitly. Dropping the handle rolls back just the same;
    /// this exists for the legacy `transaction()` wrapper.
    async fn rollback(self: Box<Self>) -> anyhow::Result<()>;
}

pub type Unit<'a> = &'a mut dyn UnitOfWork;

/// The bound a transaction body closure must satisfy: callable once with a
/// borrowed [`UnitOfWork`] for any lifetime `'a`, yielding a `Send` future for
/// that same `'a`. Routed through plain [`FnOnce`] rather than `AsyncFnOnce`,
/// which cannot currently be proven at that higher rank (rust-lang/rust#110338),
/// so call sites need no `Box::pin`.
pub trait UnitOfWorkFn<'a, T>: FnOnce(&'a mut dyn UnitOfWork) -> Self::Fut {
    /// The future `Self` returns when called — named so a `for<'a>` bound can
    /// require it `Send + 'a` without knowing the closure's concrete type.
    type Fut: Future<Output = anyhow::Result<T>> + Send + 'a;
}

impl<'a, T, F, Fut> UnitOfWorkFn<'a, T> for F
where
    F: FnOnce(&'a mut dyn UnitOfWork) -> Fut,
    Fut: Future<Output = anyhow::Result<T>> + Send + 'a,
{
    type Fut = Fut;
}
