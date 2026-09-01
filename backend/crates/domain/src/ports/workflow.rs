//! Workflow ports (DESIGN/Workflow `9895957`; Ownership Separation DD
//! `29130754`): the account-side organization surface. Reads are pool-backed;
//! writes are transaction-bound, because a card's placement and its position in
//! a list move together or not at all.
//!
//! Columns — the **Lists** of DESIGN/Workflow — carry their own pair, because a
//! column has its own id so its visibility can be set apart from its board's.
//! The column *order* is not here: it is the `Vec<ColumnId>` on the
//! [`Workflow`](crate::elements::workflow::Workflow) itself, so reordering is a
//! workflow write, not a column one.
//!

use async_trait::async_trait;

use crate::elements::{
    account::AccountId,
    commission::CommissionId,
    workflow::{Column, ColumnId, Workflow, WorkflowId, WorkflowName},
};

/// The **write** surface of an account's workflows — reachable only on an open
/// [`UnitOfWork`](crate::ports::UnitOfWork) (`uow.workflows()`), never
/// pool-backed (DD `24150017`). A board mutation is rarely one row: adding a
/// card writes the placement and its position, moving one rewrites the
/// neighbours it displaces, and a locked list's recomputed order rewrites every
/// card at once. None of those may half-land.
///
/// Everything board-local lives on the **(workflow, commission) edge** — the
/// list, the position, the per-board archived state, and workflow-scoped
/// metadata such as round-robin weights. The commission stores none of it and
/// never learns it exists.
#[async_trait]
pub trait WorkflowWrites: Send {
    async fn create(
        &mut self,
        name: &WorkflowName,
        account_id: &AccountId,
    ) -> anyhow::Result<Workflow>;
    /// Deletes workflow AND columns inside of it. NOT the commissions.
    async fn delete(&mut self, workflow_id: &WorkflowId) -> anyhow::Result<()>;
    /// Persists the board's column order as the domain holds it.
    async fn set_indexes(&mut self, workflow: &Workflow) -> anyhow::Result<()>;
}

/// The **read** surface of an account's workflows — pool-backed and
/// non-transactional. A workflow belongs to exactly one account, and an account
/// may hold many.
///
/// Reads here answer *where a card sits*, never *what a card shows*: a board
/// renders each commission through the viewer's own effective view, projected
/// per viewer at serialization, so a list holds no privacy of its own and this
/// port carries no visibility logic. Who may read the board is the caller's
/// authorization, settled before this is reached.
#[async_trait]
pub trait WorkflowStore: Send + Sync {
    async fn find(&self, workflow_id: &WorkflowId) -> anyhow::Result<Option<Workflow>>;
    async fn owning_account_of(&self, workflow_id: &WorkflowId) -> anyhow::Result<AccountId>;
    async fn columns(&self, workflow_id: &WorkflowId) -> anyhow::Result<Vec<Column>>;
}

/// The **write** surface of a workflow's columns — reachable only on an open
/// [`UnitOfWork`](crate::ports::UnitOfWork) (`uow.columns()`), never pool-backed
/// (DD `24150017`). A column rarely moves alone: removing one has to say where
/// its cards went, and renaming or reordering touches the board's own column
/// list, so the two writes land together or not at all.
#[async_trait]
pub trait ColumnWrites: Send {
    /// Delete a column
    async fn delete(&mut self, column_id: &ColumnId) -> anyhow::Result<()>;
    /// Persists the column's card list as the domain holds it.
    async fn set_commissions(&mut self, column: &Column) -> anyhow::Result<()>;
    /// Rename a column
    async fn rename(&mut self, column: &Column) -> anyhow::Result<()>;
}

/// The **read** surface of a workflow's columns — pool-backed and
/// non-transactional.
///
/// A column holds no privacy of its own beyond its own visibility: what an
/// outsider sees of its contents is each commission's projection, applied per
/// viewer at serialization. Who may read the board is the caller's
/// authorization, settled before this is reached.
#[async_trait]
pub trait ColumnStore: Send + Sync {
    async fn find(&self, column_id: &ColumnId) -> anyhow::Result<Option<Column>>;
    async fn has_commissions(&self, column_id: &ColumnId) -> anyhow::Result<bool>;
    async fn find_column(
        &self,
        workflow_id: &WorkflowId,
        commission_id: &CommissionId,
    ) -> anyhow::Result<Option<Column>>;
    async fn owning_account_of(&self, column_id: &ColumnId) -> anyhow::Result<AccountId>;
    async fn find_workflow_of(&self, column_id: &ColumnId) -> anyhow::Result<Workflow>;
}
