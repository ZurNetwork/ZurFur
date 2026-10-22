//! [`NodeStore`] (reads) and [`NodeRepo`] (the unit's reads and writes) over
//! PostgreSQL, in the `node` table. Reads are pool-backed; writes only via an open
//! [`UnitOfWork`](domain::ports::UnitOfWork) (`uow.nodes()`). A node statement's
//! failure ends the unit: [`PgUnitOfWork`](crate::PgUnitOfWork) remembers it, and
//! its commit rolls back and returns an error.
//!
//! [`NodeRepo`]: domain::ports::NodeRepo

use domain::elements::level::Level;
use domain::elements::node::{NewNode, NodeId, NodeRow, NodeSegment, NodeType};
use domain::elements::text::StoredText;
use domain::ports::{NodeReads, NodeStore, NodeWrites, SegmentTaken};
use sqlx::{PgConnection, PgExecutor, PgPool};

use crate::queries::node as sql;

/// The one unique constraint a create maps to [`SegmentTaken`].
const SEGMENT_KEY: &str = "node_parent_segment_key";

/// PostgreSQL read store for the Den's nodes (the [`NodeStore`] surface). Writes
/// live on [`PgNodeWrites`].
pub struct PgNodeStore {
    pool: PgPool,
}

impl PgNodeStore {
    /// Wraps a [`PgPool`] as a [`NodeStore`]. Clones the pool handle (an `Arc`),
    /// leaving the caller's intact.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// PostgreSQL read and write view of the Den's nodes over an open transaction.
/// Built by `uow.nodes()`; a failed statement marks the unit failed.
pub struct PgNodeWrites<'a> {
    /// The open transaction, borrowed from the [`PgUnitOfWork`](crate::PgUnitOfWork).
    pub(crate) conn: &'a mut PgConnection,
    /// The unit's failure memory: set by any failed node statement, read by its commit.
    pub(crate) failed: &'a mut bool,
}

impl PgNodeWrites<'_> {
    /// Refuses a call on a unit a node statement already ended.
    fn still_open(&self) -> anyhow::Result<()> {
        anyhow::ensure!(
            !*self.failed,
            "a refused node statement ended this unit of work"
        );
        Ok(())
    }

    /// Passes `result` through, remembering a failure on the unit.
    fn noted<T>(&mut self, result: sqlx::Result<T>) -> sqlx::Result<T> {
        if result.is_err() {
            *self.failed = true;
        }
        result
    }
}

/// The unique-violation constraint name on an sqlx database error, if any.
fn constraint_of(err: &sqlx::Error) -> Option<&str> {
    err.as_database_error()
        .and_then(|database_error| database_error.constraint())
}

/// The top-level node of `node_type` holding `segment`, on `executor`.
async fn at_top(
    executor: impl PgExecutor<'_>,
    node_type: &NodeType,
    segment: &NodeSegment,
) -> sqlx::Result<Vec<sql::NodeRow>> {
    sql::at_top(executor, node_type.into(), segment.as_ref()).await
}

/// At most one row of an `at_top` read: the segment key holds one top-level node per segment.
fn the_one(rows: Vec<sql::NodeRow>) -> anyhow::Result<Option<NodeRow>> {
    anyhow::ensure!(rows.len() <= 1, "two top-level nodes hold one segment");
    rows.into_iter().next().map(NodeRow::try_from).transpose()
}

/// Loads every row, in order.
fn loaded(rows: Vec<sql::NodeRow>) -> anyhow::Result<Vec<NodeRow>> {
    rows.into_iter().map(NodeRow::try_from).collect()
}

#[async_trait::async_trait]
impl NodeStore for PgNodeStore {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn at_top(
        &self,
        node_type: &NodeType,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>> {
        let rows = at_top(&self.pool, node_type, segment).await?;
        the_one(rows)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn child(
        &self,
        parent: &NodeId,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>> {
        let row = sql::child(&self.pool, *parent.as_ref(), segment.as_ref()).await?;
        row.map(NodeRow::try_from).transpose()
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn children(&self, parent: &NodeId) -> anyhow::Result<Vec<NodeRow>> {
        let rows = sql::children(&self.pool, *parent.as_ref()).await?;
        loaded(rows)
    }
}

#[async_trait::async_trait]
impl NodeReads for PgNodeWrites<'_> {
    #[tracing::instrument(level = "debug", skip_all)]
    async fn at_top(
        &mut self,
        node_type: &NodeType,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>> {
        self.still_open()?;
        let read = at_top(&mut *self.conn, node_type, segment).await;
        let rows = self.noted(read)?;
        the_one(rows)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn child(
        &mut self,
        parent: &NodeId,
        segment: &NodeSegment,
    ) -> anyhow::Result<Option<NodeRow>> {
        self.still_open()?;
        let read = sql::child(&mut *self.conn, *parent.as_ref(), segment.as_ref()).await;
        let row = self.noted(read)?;
        row.map(NodeRow::try_from).transpose()
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn children(&mut self, parent: &NodeId) -> anyhow::Result<Vec<NodeRow>> {
        self.still_open()?;
        let read = sql::children(&mut *self.conn, *parent.as_ref()).await;
        let rows = self.noted(read)?;
        loaded(rows)
    }
}

#[async_trait::async_trait]
impl NodeWrites for PgNodeWrites<'_> {
    /// Inserts the node. Only a violation of `node_parent_segment_key` returns
    /// [`SegmentTaken`]; every other refusal is opaque.
    #[tracing::instrument(level = "debug", skip_all)]
    async fn create(&mut self, node: &NewNode) -> anyhow::Result<()> {
        self.still_open()?;
        let insert = sql::create(
            &mut *self.conn,
            *node.id.as_ref(),
            node.parent.map(uuid::Uuid::from),
            node.segment.as_ref(),
            node.node_type.into(),
            node.level.into(),
            node.name.as_str(),
        )
        .await;
        let insert = self.noted(insert);
        if let Err(ref err) = insert
            && constraint_of(err) == Some(SEGMENT_KEY)
        {
            return Err(anyhow::Error::new(SegmentTaken));
        }
        insert?;
        Ok(())
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn set_name(&mut self, id: &NodeId, name: &StoredText) -> anyhow::Result<bool> {
        self.still_open()?;
        let update = sql::set_name(&mut *self.conn, *id.as_ref(), name.as_str()).await;
        let updated = self.noted(update)?;
        Ok(updated > 0)
    }

    #[tracing::instrument(level = "debug", skip_all)]
    async fn delete(&mut self, id: &NodeId) -> anyhow::Result<bool> {
        self.still_open()?;
        let delete = sql::delete(&mut *self.conn, *id.as_ref()).await;
        let deleted = self.noted(delete)?;
        Ok(deleted > 0)
    }
}

impl TryFrom<sql::NodeRow> for NodeRow {
    type Error = anyhow::Error;

    /// Loads a stored row. The table's checks hold the segment, type and level to
    /// their rules, so a refusal here means the row was corrupted.
    fn try_from(row: sql::NodeRow) -> anyhow::Result<Self> {
        Ok(Self {
            id: NodeId::from(row.id),
            parent: row.parent.map(NodeId::from),
            segment: NodeSegment::try_from(row.segment)?,
            node_type: row.r#type.parse::<NodeType>()?,
            level: row.level.parse::<Level>()?,
            name: StoredText::from(row.name),
        })
    }
}
