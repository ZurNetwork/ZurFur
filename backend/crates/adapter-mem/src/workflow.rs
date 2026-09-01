//! In-memory workflow ports (DESIGN/Workflow `9895957`).
//!
//! The mem mirror of `adapter-pg`'s three board tables: [`StoredWorkflow`] for
//! `workflow`, [`StoredColumn`] for `workflow_column` **and** its cards
//! (`workflow_column_commission` — a column owns its ordered card list here,
//! exactly as `ColumnWrites::set_commissions` hands it over).
//!
//! **Placement lives here, and only here.** A commission's presence on an
//! account's board IS its placement (Ownership Separation DD `29130754`
//! Decision 6); the commission side stores none of it and never learns it
//! exists.

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

/// One board as the mem backend keeps it — the mirror of a pg `workflow` row.
/// Stored as parts because [`Workflow`] owns its columns, which live in their
/// own map (as they do in their own table).
#[derive(Clone, PartialEq)]
pub(crate) struct StoredWorkflow {
    pub(crate) account_id: AccountId,
    pub(crate) name: WorkflowName,
    pub(crate) visibility: Visibility,
}

/// One column as the mem backend keeps it — the mirror of a pg
/// `workflow_column` row **plus** its `workflow_column_commission` cards, which
/// the domain's [`Column`] carries as one ordered `Vec`.
#[derive(Clone, PartialEq)]
pub(crate) struct StoredColumn {
    pub(crate) workflow_id: WorkflowId,
    pub(crate) name: ColumnName,
    pub(crate) visibility: Visibility,
    pub(crate) position: Position,
    pub(crate) commissions: Vec<CommissionId>,
}

impl StoredColumn {
    /// Rebuild the domain [`Column`] from the stored parts, through
    /// `Column::loaded` — so the mem adapter refuses the same malformed state
    /// the pg one does (a card listed twice) rather than handing back a column
    /// the domain would never have produced.
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

/// Every column of one board, in board order — the mem mirror of
/// `queries/workflow/columns.sql`'s `ORDER BY position`.
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

/// In-memory [`WorkflowWrites`] surface over the unit's **staged** backend, so
/// board mutations land only when the unit commits.
pub struct MemWorkflowWrites(pub(crate) MemBackend);

#[async_trait]
impl WorkflowWrites for MemWorkflowWrites {
    /// Mint one board for an account. The id and the closed-door default
    /// visibility are the domain's (`Workflow::new`), matching the pg adapter:
    /// a board is born `Private` and widened deliberately.
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

    /// Delete a board and its columns — the mem mirror of the pg `ON DELETE
    /// CASCADE`. Never the commissions those cards pointed at. Deleting an
    /// absent board is a no-op.
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

    /// Persist the board's column order as the domain holds it — an upsert per
    /// column, mirroring `queries/workflow/upsert_column.sql`, because
    /// `Columns::add` mints a column into the in-memory board and hands the
    /// whole board over.
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
                // The board write carries structure, never cards: a column's
                // card list is `set_commissions`'s to move, so an upsert here
                // must preserve whatever the column already holds.
                commissions: cards,
            };
            columns.insert(column.id.clone(), stored);
        }
        Ok(())
    }
}

/// In-memory [`WorkflowStore`] read surface over the **shared** [`MemBackend`].
pub struct MemWorkflowStore(pub(crate) MemBackend);

#[async_trait]
impl WorkflowStore for MemWorkflowStore {
    /// The whole board — columns in order, each with its cards — or `None`.
    async fn find(&self, workflow_id: &WorkflowId) -> anyhow::Result<Option<Workflow>> {
        workflow_of(&self.0, workflow_id)
    }

    /// The account a board belongs to. An absent board is an `Err`, matching the
    /// pg adapter: the caller asked whose board this is about a board it
    /// believes exists.
    async fn owning_account_of(&self, workflow_id: &WorkflowId) -> anyhow::Result<AccountId> {
        self.0
            .workflows
            .lock()
            .expect("MemBackend workflows mutex poisoned")
            .get(workflow_id)
            .map(|stored| stored.account_id.clone())
            .ok_or_else(|| anyhow::anyhow!("no such workflow"))
    }

    /// A board's columns in board order, each with its cards.
    async fn columns(&self, workflow_id: &WorkflowId) -> anyhow::Result<Vec<Column>> {
        columns_of(&self.0, workflow_id)
    }
}

/// In-memory [`ColumnWrites`] surface over the unit's **staged** backend.
pub struct MemColumnWrites(pub(crate) MemBackend);

#[async_trait]
impl ColumnWrites for MemColumnWrites {
    /// Delete one column and, with it, its cards — never the commissions. The
    /// caller refuses a column that still holds cards, so this is a backstop.
    async fn delete(&mut self, column_id: &ColumnId) -> anyhow::Result<()> {
        self.0
            .columns
            .lock()
            .expect("MemBackend columns mutex poisoned")
            .remove(column_id);
        Ok(())
    }

    /// Persist the column's card list as the domain holds it — a wholesale
    /// replacement, mirroring the pg clear-then-re-place, because
    /// `Column.commissions` is an ordered `Vec` with no per-card key.
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

    /// Rename one column. The board-level duplicate-name check is
    /// `Workflow::rename_column`'s, already made before this is reached.
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

/// In-memory [`ColumnStore`] read surface over the **shared** [`MemBackend`].
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

    /// Which column of `workflow_id` holds `commission_id`, or `None`.
    /// Deliberately scoped by board — a commission sits in at most one column
    /// per board, and on as many boards as care to position it.
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

    /// The account that owns the board this column sits on. An absent column is
    /// an `Err`, matching [`WorkflowStore::owning_account_of`].
    async fn owning_account_of(&self, column_id: &ColumnId) -> anyhow::Result<AccountId> {
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

        MemWorkflowStore(self.0.clone())
            .owning_account_of(&workflow_id)
            .await
    }

    /// The whole board a column sits on. An absent column — or one whose board
    /// has gone — is an `Err`.
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
