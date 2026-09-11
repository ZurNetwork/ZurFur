//! Workflows over PostgreSQL (DESIGN/Workflow 9895957): board writes via
//! [`PgWorkflowWrites`] on an open [`UnitOfWork`](domain::ports::UnitOfWork)
//! (`uow.workflows()`, DD 24150017); reads via the pool-backed
//! [`PgWorkflowStore`]. Placement lives here only — a commission's presence
//! on a board IS its placement (DD 29130754).

use domain::{
    elements::{
        account::AccountId,
        commission::{CommissionId, Visibility},
        did::Did,
        workflow::{Column, ColumnId, LexOrdering, Position, Workflow, WorkflowId, WorkflowName},
    },
    ports::{ColumnStore, ColumnWrites, WorkflowStore, WorkflowWrites},
};
use sqlx::{PgConnection, PgPool};

use crate::queries::{column as column_sql, workflow as workflow_sql};

/// Re-validate a stored `visibility` token into its [`Visibility`]. An `Err`
/// on a tampered or unmigrated token, never a silently-widened board.
fn to_visibility(token: &str) -> anyhow::Result<Visibility> {
    Visibility::try_from(token).map_err(|_| anyhow::anyhow!("unknown visibility token {token:?}"))
}

/// Re-validate a stored column key into its [`Position`]. An `Err` on a
/// value outside the base-62 alphabet means row tampering.
fn to_position(raw: &str) -> anyhow::Result<Position> {
    raw.parse::<Position>()
        .map_err(|err| anyhow::anyhow!("unusable column position {raw:?}: {err}"))
}

/// Load one column's cards, in board order, and rebuild the domain [`Column`].
/// Two reads, not a join: the column may legitimately hold no cards.
async fn load_column(
    pool: &PgPool,
    id: ColumnId,
    workflow_id: WorkflowId,
    name: String,
    visibility: &str,
    position: &str,
) -> anyhow::Result<Column> {
    let cards = column_sql::cards(pool, *id).await?;
    let commissions = cards.into_iter().map(CommissionId::new).collect();
    let name = name
        .parse::<WorkflowName>()
        .map_err(|_| anyhow::anyhow!("unusable stored column name"))?;

    Column::loaded(
        id,
        workflow_id,
        name,
        to_visibility(visibility)?,
        to_position(position)?,
        commissions,
    )
    .map_err(|err| anyhow::anyhow!("stored column is not a valid board list: {err}"))
}

/// Load every column of a board, in board order, each with its cards.
async fn load_columns(pool: &PgPool, workflow_id: &WorkflowId) -> anyhow::Result<Vec<Column>> {
    let rows = workflow_sql::columns(pool, **workflow_id).await?;

    let mut columns = Vec::with_capacity(rows.len());
    for row in rows {
        let column = load_column(
            pool,
            ColumnId::from(row.id),
            workflow_id.clone(),
            row.name,
            &row.visibility,
            &row.position,
        )
        .await?;
        columns.push(column);
    }
    Ok(columns)
}

/// Load one board whole — its own row, then its columns and their cards.
async fn load_workflow(pool: &PgPool, id: &WorkflowId) -> anyhow::Result<Option<Workflow>> {
    let Some(row) = workflow_sql::find(pool, **id).await? else {
        return Ok(None);
    };
    let columns = load_columns(pool, id).await?;
    let name = row
        .name
        .parse::<WorkflowName>()
        .map_err(|_| anyhow::anyhow!("unusable stored workflow name"))?;

    let workflow = Workflow::loaded(
        id.clone(),
        name,
        AccountId::new(Did::new(row.account_id)),
        to_visibility(&row.visibility)?,
        columns,
    )
    .map_err(|err| anyhow::anyhow!("stored workflow is not a valid board: {err}"))?;

    Ok(Some(workflow))
}

/// PostgreSQL board-write view over an open transaction (the [`WorkflowWrites`]
/// surface). Holds only a borrowed `&mut PgConnection`, so a pool-backed write
/// is unrepresentable. Built by `uow.workflows()`.
pub struct PgWorkflowWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait::async_trait]
impl WorkflowWrites for PgWorkflowWrites<'_> {
    /// Mints one board for an account, born `Private` (closed-door default,
    /// never widened by omission).
    async fn create(
        &mut self,
        name: &WorkflowName,
        account_id: &AccountId,
    ) -> anyhow::Result<Workflow> {
        let workflow = Workflow::new(name.clone(), account_id.clone(), Visibility::Private);

        workflow_sql::create(
            &mut *self.conn,
            *workflow.id,
            account_id.as_str(),
            name,
            workflow.visibility.as_str(),
        )
        .await?;

        Ok(workflow)
    }

    /// Deletes a board and, by cascade, its columns and cards — never the
    /// commissions they pointed at. No-op on an absent board.
    async fn delete(&mut self, workflow_id: &WorkflowId) -> anyhow::Result<()> {
        workflow_sql::delete(&mut *self.conn, **workflow_id).await?;
        Ok(())
    }

    /// Persists the board's column order as the domain holds it — an upsert per
    /// column (not an update), so a new column and the neighbours its insert
    /// displaced land on the same open unit.
    async fn set_indexes(&mut self, workflow: &Workflow) -> anyhow::Result<()> {
        for column in workflow.iter() {
            workflow_sql::upsert_column(
                &mut *self.conn,
                *column.id,
                *workflow.id,
                &column.name,
                column.visibility.as_str(),
                column.position.as_ref(),
            )
            .await?;
        }
        Ok(())
    }
}

/// PostgreSQL read store for an account's workflows (the [`WorkflowStore`]
/// surface). Holds the pool directly — board reads pay no transaction tax.
pub struct PgWorkflowStore {
    pool: PgPool,
}

impl PgWorkflowStore {
    /// Bind the store to the pool the composition root owns.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl WorkflowStore for PgWorkflowStore {
    /// Rebuilds the whole board — columns in order, each with its cards — or
    /// `None`. Goes through `Workflow::loaded`, so a row set that isn't a valid
    /// board (bad keys, duplicate names) surfaces as an `Err`.
    async fn find(&self, workflow_id: &WorkflowId) -> anyhow::Result<Option<Workflow>> {
        load_workflow(&self.pool, workflow_id).await
    }

    /// The account a board belongs to. An absent board is an `Err`, not `None`
    /// — the caller believes this board exists.
    async fn owning_account_of(&self, workflow_id: &WorkflowId) -> anyhow::Result<AccountId> {
        let did = workflow_sql::owning_account(&self.pool, **workflow_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no such workflow"))?;

        Ok(AccountId::new(Did::new(did)))
    }

    /// A board's columns in board order, each with its cards.
    async fn columns(&self, workflow_id: &WorkflowId) -> anyhow::Result<Vec<Column>> {
        load_columns(&self.pool, workflow_id).await
    }
}

/// PostgreSQL column-write view over an open transaction (the [`ColumnWrites`]
/// surface). Built by `uow.columns()`.
pub struct PgColumnWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait::async_trait]
impl ColumnWrites for PgColumnWrites<'_> {
    /// Deletes one column; card edges cascade. No-op on an absent column.
    async fn delete(&mut self, column_id: &ColumnId) -> anyhow::Result<()> {
        column_sql::delete(&mut *self.conn, **column_id).await?;
        Ok(())
    }

    /// Persists the column's card list wholesale: clear, then re-place each
    /// card at its index. Both halves run on the open unit; the
    /// `(column_id, position)` unique constraint is DEFERRABLE, checked at COMMIT.
    async fn set_commissions(&mut self, column: &Column) -> anyhow::Result<()> {
        column_sql::clear_cards(&mut *self.conn, *column.id).await?;

        for (index, commission) in column.iter().enumerate() {
            let position = i32::try_from(index)
                .map_err(|_| anyhow::anyhow!("column holds more cards than a position can hold"))?;

            column_sql::add_card(&mut *self.conn, *column.id, **commission, position).await?;
        }
        Ok(())
    }

    /// Renames one column; `(workflow_id, name)` is the store-level backstop
    /// for the board-level uniqueness check already made.
    async fn rename(&mut self, column: &Column) -> anyhow::Result<()> {
        column_sql::rename(&mut *self.conn, *column.id, &column.name).await?;
        Ok(())
    }
}

/// PostgreSQL read store for a workflow's columns (the [`ColumnStore`] surface).
/// Holds the pool directly — column reads pay no transaction tax.
pub struct PgColumnStore {
    pool: PgPool,
}

impl PgColumnStore {
    /// Bind the store to the pool the composition root owns.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl ColumnStore for PgColumnStore {
    /// One column with its cards, or `None` if no such column exists.
    async fn find(&self, column_id: &ColumnId) -> anyhow::Result<Option<Column>> {
        let Some(row) = column_sql::find(&self.pool, **column_id).await? else {
            return Ok(None);
        };

        let column = load_column(
            &self.pool,
            column_id.clone(),
            WorkflowId::from(row.workflow_id),
            row.name,
            &row.visibility,
            &row.position,
        )
        .await?;

        Ok(Some(column))
    }

    /// Whether a column still holds any card — the gate on deleting one.
    async fn has_commissions(&self, column_id: &ColumnId) -> anyhow::Result<bool> {
        column_sql::has_commissions(&self.pool, **column_id)
            .await
            .map_err(Into::into)
    }

    /// Which column of `workflow_id` holds `commission_id`, or `None`.
    /// Scoped by board: a commission sits in at most one column per board,
    /// on as many boards as position it.
    async fn find_column(
        &self,
        workflow_id: &WorkflowId,
        commission_id: &CommissionId,
    ) -> anyhow::Result<Option<Column>> {
        // Takes the first row rather than LIMIT 1, to not hide a genuine duplicate.
        let rows =
            column_sql::find_by_commission(&self.pool, **workflow_id, **commission_id).await?;
        let Some(row) = rows.into_iter().next() else {
            return Ok(None);
        };

        let column = load_column(
            &self.pool,
            ColumnId::from(row.id),
            WorkflowId::from(row.workflow_id),
            row.name,
            &row.visibility,
            &row.position,
        )
        .await?;

        Ok(Some(column))
    }

    /// The account owning the board this column sits on. An absent column is
    /// an `Err`, matching [`WorkflowStore::owning_account_of`].
    async fn owning_account_of(&self, column_id: &ColumnId) -> anyhow::Result<AccountId> {
        let did = column_sql::owning_account(&self.pool, **column_id)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("no such column"))?;

        Ok(AccountId::new(Did::new(did)))
    }

    /// The whole board a column sits on. An absent column or board is an
    /// `Err` (same reasoning as [`owning_account_of`](Self::owning_account_of)).
    async fn find_workflow_of(&self, column_id: &ColumnId) -> anyhow::Result<Workflow> {
        let row = column_sql::find(&self.pool, **column_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no such column"))?;

        load_workflow(&self.pool, &WorkflowId::from(row.workflow_id))
            .await?
            .ok_or_else(|| anyhow::anyhow!("no such workflow"))
    }
}
