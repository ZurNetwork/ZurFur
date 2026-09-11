//! In-memory workflow ports: the mem mirror of `adapter-pg`'s three board
//! tables, with a column owning its ordered card list.
//!
//! Placement lives here and only here — a commission's presence on an account's
//! board IS its placement; the commission side stores none of it. (DD 29130754)

use async_trait::async_trait;
use domain::{
    elements::{
        account::AccountId,
        commission::{CommissionId, Visibility},
        workflow::{
            Column, ColumnId, ColumnName, LexOrdering, Position, Workflow, WorkflowId, WorkflowName,
        },
    },
    ports::{ColumnStore, ColumnWrites, WorkflowStore, WorkflowWrites},
};

use crate::MemBackend;

/// One board as the mem backend keeps it. Stored as parts because [`Workflow`]
/// owns its columns, which live in their own map.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredWorkflow {
    pub(crate) account_id: AccountId,
    pub(crate) name: WorkflowName,
    pub(crate) visibility: Visibility,
}

/// One column as the mem backend keeps it — a `workflow_column` row plus its
/// cards, which the domain's [`Column`] carries as one ordered `Vec`.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredColumn {
    pub(crate) workflow_id: WorkflowId,
    pub(crate) name: ColumnName,
    pub(crate) visibility: Visibility,
    pub(crate) position: Position,
    pub(crate) commissions: Vec<CommissionId>,
}

impl StoredColumn {
    /// Rebuild the domain [`Column`] through `Column::loaded`, so malformed
    /// state (a card listed twice) is refused here as it is in pg.
    fn rebuild(&self, id: ColumnId) -> anyhow::Result<Column> {
        Column::loaded(
            id,
            self.workflow_id.clone(),
            self.name.clone(),
            self.visibility.clone(),
            self.position.clone(),
            self.commissions.clone(),
        )
        .map_err(|err| anyhow::anyhow!("stored column is not a valid board list: {err}"))
    }
}

/// Every column of one board, in board order.
fn columns_of(backend: &MemBackend, workflow_id: &WorkflowId) -> anyhow::Result<Vec<Column>> {
    let columns = backend
        .columns
        .lock()
        .expect("MemBackend columns mutex poisoned");

    let mut found = columns
        .iter()
        .filter(|(_, stored): &(&ColumnId, &StoredColumn)| stored.workflow_id == *workflow_id)
        .map(|(id, stored): (&ColumnId, &StoredColumn)| stored.rebuild(id.clone()))
        .collect::<anyhow::Result<Vec<Column>>>()?;

    found.sort_by(|a, b| a.position.cmp(&b.position));
    Ok(found)
}

/// One board whole — its own row plus its ordered, card-filled columns.
fn workflow_of(backend: &MemBackend, id: &WorkflowId) -> anyhow::Result<Option<Workflow>> {
    let stored = {
        let workflows = backend
            .workflows
            .lock()
            .expect("MemBackend workflows mutex poisoned");
        let Some(stored) = workflows.get(id) else {
            return Ok(None);
        };
        stored.clone()
    };

    let columns = columns_of(backend, id)?;
    let workflow = Workflow::loaded(
        id.clone(),
        stored.name,
        stored.account_id,
        stored.visibility,
        columns,
    )
    .map_err(|err| anyhow::anyhow!("stored workflow is not a valid board: {err}"))?;

    Ok(Some(workflow))
}

/// In-memory [`WorkflowWrites`] over the unit's staged backend, so board
/// mutations land only on commit.
pub struct MemWorkflowWrites(pub(crate) MemBackend);

#[async_trait]
impl WorkflowWrites for MemWorkflowWrites {
    /// Mint one board for an account; the id and the `Private` default come
    /// from `Workflow::new`.
    async fn create(
        &mut self,
        name: &WorkflowName,
        account_id: &AccountId,
    ) -> anyhow::Result<Workflow> {
        let workflow = Workflow::new(name.clone(), account_id.clone(), Visibility::Private);
        let stored = StoredWorkflow {
            account_id: account_id.clone(),
            name: name.clone(),
            visibility: workflow.visibility.clone(),
        };

        self.0
            .workflows
            .lock()
            .expect("MemBackend workflows mutex poisoned")
            .insert(workflow.id.clone(), stored);

        Ok(workflow)
    }

    /// Delete a board and its columns, never the commissions their cards
    /// pointed at. Deleting an absent board is a no-op.
    async fn delete(&mut self, workflow_id: &WorkflowId) -> anyhow::Result<()> {
        self.0
            .workflows
            .lock()
            .expect("MemBackend workflows mutex poisoned")
            .remove(workflow_id);

        self.0
            .columns
            .lock()
            .expect("MemBackend columns mutex poisoned")
            .retain(|_, stored| stored.workflow_id != *workflow_id);

        Ok(())
    }

    /// Persist the board's column order as the domain holds it: one upsert per
    /// column, since the caller hands the whole board over.
    async fn set_indexes(&mut self, workflow: &Workflow) -> anyhow::Result<()> {
        let mut columns = self
            .0
            .columns
            .lock()
            .expect("MemBackend columns mutex poisoned");

        for column in workflow.iter() {
            let cards = columns
                .get(&column.id)
                .map(|stored| stored.commissions.clone())
                .unwrap_or_default();

            let stored = StoredColumn {
                workflow_id: workflow.id.clone(),
                name: column.name.clone(),
                visibility: column.visibility.clone(),
                position: column.position.clone(),
                // The board write carries structure, never cards — an upsert
                // preserves whatever the column already holds.
                commissions: cards,
            };
            columns.insert(column.id.clone(), stored);
        }
        Ok(())
    }
}

/// In-memory [`WorkflowStore`] read surface over the shared [`MemBackend`].
pub struct MemWorkflowStore(pub(crate) MemBackend);

#[async_trait]
impl WorkflowStore for MemWorkflowStore {
    /// The whole board — columns in order, each with its cards — or `None`.
    async fn find(&self, workflow_id: &WorkflowId) -> anyhow::Result<Option<Workflow>> {
        workflow_of(&self.0, workflow_id)
    }

    /// The account a board belongs to, or `None` if no such board exists.
    async fn owning_account_of(
        &self,
        workflow_id: &WorkflowId,
    ) -> anyhow::Result<Option<AccountId>> {
        Ok(self
            .0
            .workflows
            .lock()
            .expect("MemBackend workflows mutex poisoned")
            .get(workflow_id)
            .map(|stored| stored.account_id.clone()))
    }

    /// A board's columns in board order, each with its cards.
    async fn columns(&self, workflow_id: &WorkflowId) -> anyhow::Result<Vec<Column>> {
        columns_of(&self.0, workflow_id)
    }
}

/// In-memory [`ColumnWrites`] surface over the unit's staged backend.
pub struct MemColumnWrites(pub(crate) MemBackend);

#[async_trait]
impl ColumnWrites for MemColumnWrites {
    /// Delete one column and its cards, never the commissions. A backstop —
    /// the caller already refuses a column that still holds cards.
    async fn delete(&mut self, column_id: &ColumnId) -> anyhow::Result<()> {
        self.0
            .columns
            .lock()
            .expect("MemBackend columns mutex poisoned")
            .remove(column_id);
        Ok(())
    }

    /// Persist the column's card list as the domain holds it: a wholesale
    /// replacement, since the list has no per-card key.
    async fn set_commissions(&mut self, column: &Column) -> anyhow::Result<()> {
        let mut columns = self
            .0
            .columns
            .lock()
            .expect("MemBackend columns mutex poisoned");

        let stored = columns
            .get_mut(&column.id)
            .ok_or_else(|| anyhow::anyhow!("no such column"))?;

        stored.commissions = column.iter().cloned().collect();
        Ok(())
    }

    /// Rename one column; the duplicate-name check happens before this.
    async fn rename(&mut self, column: &Column) -> anyhow::Result<()> {
        let mut columns = self
            .0
            .columns
            .lock()
            .expect("MemBackend columns mutex poisoned");

        let stored = columns
            .get_mut(&column.id)
            .ok_or_else(|| anyhow::anyhow!("no such column"))?;

        stored.name = column.name.clone();
        Ok(())
    }
}

/// In-memory [`ColumnStore`] read surface over the shared [`MemBackend`].
pub struct MemColumnStore(pub(crate) MemBackend);

#[async_trait]
impl ColumnStore for MemColumnStore {
    /// One column with its cards, or `None`.
    async fn find(&self, column_id: &ColumnId) -> anyhow::Result<Option<Column>> {
        let columns = self
            .0
            .columns
            .lock()
            .expect("MemBackend columns mutex poisoned");

        columns
            .get(column_id)
            .map(|stored: &StoredColumn| stored.rebuild(column_id.clone()))
            .transpose()
    }

    /// Whether a column still holds any card — the gate on deleting one.
    async fn has_commissions(&self, column_id: &ColumnId) -> anyhow::Result<bool> {
        Ok(self
            .0
            .columns
            .lock()
            .expect("MemBackend columns mutex poisoned")
            .get(column_id)
            .is_some_and(|stored| !stored.commissions.is_empty()))
    }

    /// Which column of `workflow_id` holds `commission_id`, or `None`. Scoped
    /// by board: a commission sits in at most one column per board.
    async fn find_column(
        &self,
        workflow_id: &WorkflowId,
        commission_id: &CommissionId,
    ) -> anyhow::Result<Option<Column>> {
        let columns = self
            .0
            .columns
            .lock()
            .expect("MemBackend columns mutex poisoned");

        columns
            .iter()
            .find(|(_, stored): &(&ColumnId, &StoredColumn)| {
                stored.workflow_id == *workflow_id && stored.commissions.contains(commission_id)
            })
            .map(|(id, stored): (&ColumnId, &StoredColumn)| stored.rebuild(id.clone()))
            .transpose()
    }

    /// The account that owns the board this column sits on, or `None` if no
    /// such column exists.
    async fn owning_account_of(&self, column_id: &ColumnId) -> anyhow::Result<Option<AccountId>> {
        let stored_workflow_id = self
            .0
            .columns
            .lock()
            .expect("MemBackend columns mutex poisoned")
            .get(column_id)
            .map(|stored| stored.workflow_id.clone());

        let Some(workflow_id) = stored_workflow_id else {
            return Ok(None);
        };

        MemWorkflowStore(self.0.clone())
            .owning_account_of(&workflow_id)
            .await
    }

    /// The whole board a column sits on; an absent column or board is an `Err`.
    async fn find_workflow_of(&self, column_id: &ColumnId) -> anyhow::Result<Workflow> {
        let workflow_id = {
            let columns = self
                .0
                .columns
                .lock()
                .expect("MemBackend columns mutex poisoned");
            columns
                .get(column_id)
                .map(|stored| stored.workflow_id.clone())
                .ok_or_else(|| anyhow::anyhow!("no such column"))?
        };

        workflow_of(&self.0, &workflow_id)?.ok_or_else(|| anyhow::anyhow!("no such workflow"))
    }
}
