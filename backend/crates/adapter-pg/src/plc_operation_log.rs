//! [`PgPlcOperationLog`] — PostgreSQL append-only log of the `did:plc` operations
//! Zurfur has submitted for each minted account identity.
//!
//! Implements the [`PlcOperationLog`] port over the `plc_operations` table. A DID is
//! a chain of operations; every non-genesis operation references the CID of the DID's
//! most recent operation as its `prev`. Because v1 does not fetch the chain back from
//! the (gated) canonical directory, this log is Zurfur's own record — used to chain
//! the next operation and to audit what it published (ZMVP-34, DD `23003138`; reused
//! by ZMVP-50/51). Pool-backed and single-row like [`crate::PgKeyStore`]: it is
//! written during minting (genesis) and hard-delete (tombstone), both outside the
//! account [`UnitOfWork`](domain::ports::UnitOfWork).

use async_trait::async_trait;
use chrono::Utc;
use domain::{
    elements::{did::Did, plc_operation::PlcOperationRecord},
    ports::PlcOperationLog,
};
use sqlx::{PgPool, query};

/// PostgreSQL [`PlcOperationLog`]: appends operation rows and reads back a DID's most
/// recent CID. Holds the pool directly (cheap to clone).
pub struct PgPlcOperationLog {
    pool: PgPool,
}

impl PgPlcOperationLog {
    /// Build the log over a connection `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PlcOperationLog for PgPlcOperationLog {
    /// Insert one operation row. `operation_json` (public material only) is stored as
    /// `jsonb`; `seq` and `created_at` order the chain. The `cid` unique index makes a
    /// duplicate append a constraint error, surfaced to the caller.
    async fn append(&self, record: &PlcOperationRecord) -> anyhow::Result<()> {
        // The record carries the op as JSON text (the domain crate has no serde_json);
        // parse it here so it lands as native `jsonb`.
        let operation: serde_json::Value = serde_json::from_str(&record.operation_json)?;
        query!(
            r#"
            INSERT INTO plc_operations (did, cid, "type", prev, operation, created_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            record.did.as_str(),
            record.cid,
            record.op_type,
            record.prev,
            operation,
            Utc::now(),
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// The `cid` of the DID's highest-`seq` (most recent) operation, or `None`.
    async fn latest_cid(&self, did: &Did) -> anyhow::Result<Option<String>> {
        let row = query!(
            r#"
            SELECT cid AS "cid!"
            FROM plc_operations
            WHERE did = $1
            ORDER BY seq DESC
            LIMIT 1
            "#,
            did.as_str(),
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| row.cid))
    }

    /// The DID's highest-`seq` (most recent) operation as a full record, or `None`.
    /// `operation` is stored as `jsonb`; it is re-serialized to the JSON text the
    /// record carries. Used by an update to carry the prior op's public fields
    /// forward without touching custody's non-signing private keys (F2).
    async fn latest_op(&self, did: &Did) -> anyhow::Result<Option<PlcOperationRecord>> {
        let row = query!(
            r#"
            SELECT cid AS "cid!", "type" AS "op_type!", prev, operation AS "operation!"
            FROM plc_operations
            WHERE did = $1
            ORDER BY seq DESC
            LIMIT 1
            "#,
            did.as_str(),
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| PlcOperationRecord {
            did: did.clone(),
            cid: row.cid,
            op_type: row.op_type,
            prev: row.prev,
            operation_json: row.operation.to_string(),
        }))
    }
}
