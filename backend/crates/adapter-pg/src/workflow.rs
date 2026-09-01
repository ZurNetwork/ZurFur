//! Workflows over PostgreSQL (DESIGN/Workflow `9895957`): board mutations via
//! [`PgWorkflowWrites`] — reachable only on an open
//! [`UnitOfWork`](domain::ports::UnitOfWork) (`uow.workflows()`), so a card and
//! the neighbours its move displaces land together (DD `24150017`) — and the
//! board read via the pool-backed [`PgWorkflowStore`].
//!
//! **Placement lives here, and only here.** A commission's presence on an
//! account's board IS its placement (Ownership Separation DD `29130754`
//! Decision 6, "placement = workflow membership rows, account-side"); there is
//! no second account-level claim, which is why the same commission can sit on N
//! boards with no conflict and why nothing in `queries/commission/` positions
//! anything any more.
//!
//! The SQL lives in `queries/workflow/` and `queries/column/`; the typed
//! functions and row shapes are generated against the migrated schema (see
//! [`crate::queries`]).

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

/// Re-validate a stored `visibility` token into its [`Visibility`] — the gate
/// every board read passes through, so a tampered or unmigrated token surfaces
/// as an `Err` rather than a silently-widened board.
fn to_visibility(token: &str) -> anyhow::Result<Visibility> {
    Visibility::try_from(token).map_err(|_| anyhow::anyhow!("unknown visibility token {token:?}"))
}

/// Re-validate a stored column key into its [`Position`]. The domain mints these
/// and compares them bytewise; a value outside the base-62 alphabet (or one
/// ending in `0`) means row tampering, never a default.
fn to_position(raw: &str) -> anyhow::Result<Position> {
    raw.parse::<Position>()
        .map_err(|err| anyhow::anyhow!("unusable column position {raw:?}: {err}"))
}

/// Load one column's cards, in board order, and rebuild the domain [`Column`].
///
/// Two reads rather than a join because a column legitimately holds no cards:
/// the row is the column, the cards are a set over it, and `Column::loaded`
/// wants them already ordered.
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
/// surface). Holds **only** a borrowed `&mut PgConnection` — the transaction
/// owned by the [`PgUnitOfWork`](crate::PgUnitOfWork) — so no pool is in scope
/// here and a pool-backed board write is unrepresentable. Built by
/// `uow.workflows()`.
pub struct PgWorkflowWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    /// Writes execute on `&mut *self.conn`; there is deliberately no pool here.
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait::async_trait]
impl WorkflowWrites for PgWorkflowWrites<'_> {
    /// Mint one board for an account. The id and the **closed-door default**
    /// visibility are the domain's (`Workflow::new`) — Zurfur is closed-door, so
    /// a board is born `Private` and is widened deliberately, never by omission
    /// (DESIGN/Workflow, "Default visibility").
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

    /// Delete a board and, by cascade, its columns and their cards — never the
    /// commissions those cards pointed at. Deleting an absent board matches no
    /// row and is a no-op, which keeps a lost race idempotent rather than an
    /// error.
    async fn delete(&mut self, workflow_id: &WorkflowId) -> anyhow::Result<()> {
        workflow_sql::delete(&mut *self.conn, **workflow_id).await?;
        Ok(())
    }

    /// Persist the board's column order as the domain holds it.
    ///
    /// An upsert per column, not an update: `Columns::add` mints a column into
    /// the in-memory board and hands the **whole board** over, so a new column
    /// and the keys of the neighbours it displaced arrive through this one path.
    /// All of them land on the open unit, so a half-reordered board is
    /// unrepresentable.
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
    /// Rebuild the whole board — its columns in order, each with its cards — or
    /// `None` if no such board exists. Rehydration goes through
    /// `Workflow::loaded`, so stored rows that do not form a valid board (keys
    /// out of order, a duplicated column name) surface as an `Err` instead of
    /// rendering wrong.
    async fn find(&self, workflow_id: &WorkflowId) -> anyhow::Result<Option<Workflow>> {
        load_workflow(&self.pool, workflow_id).await
    }

    /// The account a board belongs to — the authorization lookup a board
    /// mutation makes before touching anything. An absent board is an `Err`,
    /// not a `None`: the caller asked whose board this is about a board it
    /// believes exists.
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
/// surface). Holds **only** a borrowed `&mut PgConnection`, so a pool-backed
/// column write is unrepresentable. Built by `uow.columns()`.
pub struct PgColumnWrites<'a> {
    /// The open transaction, borrowed from the [`UnitOfWork`](domain::ports::UnitOfWork).
    pub(crate) conn: &'a mut PgConnection,
}

#[async_trait::async_trait]
impl ColumnWrites for PgColumnWrites<'_> {
    /// Delete one column; its card edges cascade with it. The caller refuses a
    /// column that still holds cards, so the cascade is a backstop rather than
    /// the path. An absent column matches nothing and is a no-op.
    async fn delete(&mut self, column_id: &ColumnId) -> anyhow::Result<()> {
        column_sql::delete(&mut *self.conn, **column_id).await?;
        Ok(())
    }

    /// Persist the column's card list as the domain holds it — clear, then
    /// re-place each card at its index.
    ///
    /// A **wholesale rewrite**, matching the domain: `Column.commissions` is an
    /// ordered `Vec` with no per-card key, so there is no single-card edit to
    /// express. Both halves run on the open unit, so the board is never observed
    /// empty, and the `(column_id, position)` unique constraint is DEFERRABLE —
    /// the rewrite may pass through duplicate indexes and is checked once at
    /// COMMIT, when the list must be a clean `0..n` again.
    async fn set_commissions(&mut self, column: &Column) -> anyhow::Result<()> {
        column_sql::clear_cards(&mut *self.conn, *column.id).await?;

        for (index, commission) in column.iter().enumerate() {
            let position = i32::try_from(index)
                .map_err(|_| anyhow::anyhow!("column holds more cards than a position can hold"))?;

            column_sql::add_card(&mut *self.conn, *column.id, **commission, position).await?;
        }
        Ok(())
    }

    /// Rename one column. The `(workflow_id, name)` unique constraint is the
    /// store-level backstop for the board-level check `Workflow::rename_column`
    /// already made.
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

    /// Whether a column still holds any card — the gate on deleting one, so
    /// removing a list never silently drops the cards on it.
    async fn has_commissions(&self, column_id: &ColumnId) -> anyhow::Result<bool> {
        column_sql::has_commissions(&self.pool, **column_id)
            .await
            .map_err(Into::into)
    }

    /// Which column of `workflow_id` holds `commission_id`, or `None`.
    ///
    /// Deliberately scoped by board: a commission sits in at most one column
    /// **per board**, and on as many boards as care to position it — the NxM the
    /// Ownership Separation DD makes native — so there is no such thing as "the"
    /// column of a commission.
    async fn find_column(
        &self,
        workflow_id: &WorkflowId,
        commission_id: &CommissionId,
    ) -> anyhow::Result<Option<Column>> {
        // The join cannot prove single-row to the query planner, though the
        // (column_id, commission_id) primary key plus the board scope makes it
        // one: take the first rather than a `LIMIT 1` that would hide a genuine
        // duplicate behind a silent truncation.
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

    /// The account that owns the board this column sits on — the authorization
    /// lookup a column mutation makes when it holds only the column's id. An
    /// absent column is an `Err`, matching
    /// [`WorkflowStore::owning_account_of`].
    async fn owning_account_of(&self, column_id: &ColumnId) -> anyhow::Result<AccountId> {
        let did = column_sql::owning_account(&self.pool, **column_id)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("no such column"))?;

        Ok(AccountId::new(Did::new(did)))
    }

    /// The whole board a column sits on. An absent column — or a column whose
    /// board has gone — is an `Err`, for the same reason as
    /// [`owning_account_of`](Self::owning_account_of).
    async fn find_workflow_of(&self, column_id: &ColumnId) -> anyhow::Result<Workflow> {
        let row = column_sql::find(&self.pool, **column_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("no such column"))?;

        load_workflow(&self.pool, &WorkflowId::from(row.workflow_id))
            .await?
            .ok_or_else(|| anyhow::anyhow!("no such workflow"))
    }
}
