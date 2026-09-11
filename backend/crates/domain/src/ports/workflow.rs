//! Workflow ports: the account-side organization surface. Reads are pool-backed,
//! writes transaction-bound. Columns — the Lists of DESIGN/Workflow — carry their
//! own pair, since a column has its own id and visibility; column *order* lives
//! on the [`Workflow`] itself, so reordering
//! is a workflow write. (Workflow 9895957)

use async_trait::async_trait;

use crate::elements::{
    account::AccountId,
    commission::CommissionId,
    workflow::{Column, ColumnId, Workflow, WorkflowId, WorkflowName},
};

/// The **write** surface of an account's workflows — reachable only on an open
/// [`UnitOfWork`](crate::ports::UnitOfWork) (`uow.workflows()`), because a board
/// mutation is rarely one row and none of it may half-land. Everything
/// board-local lives on the (workflow, commission) edge; the commission stores
/// none of it and never learns it exists.
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
/// non-transactional. A workflow belongs to exactly one account. These reads
/// answer where a card sits, never what it shows: visibility is projected per
/// viewer at serialization, so this port carries none. Who may read the board is
/// the caller's authorization.
#[async_trait]
pub trait WorkflowStore: Send + Sync {
    async fn find(&self, workflow_id: &WorkflowId) -> anyhow::Result<Option<Workflow>>;
    async fn owning_account_of(&self, workflow_id: &WorkflowId) -> anyhow::Result<AccountId>;
    async fn columns(&self, workflow_id: &WorkflowId) -> anyhow::Result<Vec<Column>>;
}

/// The **write** surface of a workflow's columns — reachable only on an open
/// [`UnitOfWork`](crate::ports::UnitOfWork) (`uow.columns()`). A column rarely
/// moves alone: removing one has to say where its cards went, and reordering
/// touches the board's own column list.
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
/// non-transactional. Beyond its own visibility a column holds no privacy: its
/// contents are each commission's projection, applied per viewer at
/// serialization. Who may read the board is the caller's authorization.
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
