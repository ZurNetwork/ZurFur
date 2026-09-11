//! [`PgPlcOperationLog`] — PostgreSQL append-only log of the `did:plc`
//! operations submitted for each minted account identity, over
//! `plc_operations`. Every non-genesis operation references the CID of the
//! prior one as `prev`; this is Zurfur's own chain record (v1 doesn't fetch it
//! back from the canonical directory). Pool-backed, outside the account
//! [`UnitOfWork`](domain::ports::UnitOfWork), like [`crate::PgKeyStore`].

use async_trait::async_trait;
use chrono::Utc;
use domain::{
    elements::{did::Did, plc_operation::PlcOperationRecord},
    ports::PlcOperationLog,
};
use sqlx::PgPool;

use crate::queries::plc as sql;

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
    /// Inserts one operation row as `jsonb`; `seq`/`created_at` order the
    /// chain. The `cid` unique index makes a duplicate append a constraint error.
    async fn append(&self, record: &PlcOperationRecord) -> anyhow::Result<()> {
        // The record carries the op as JSON text (`PlcOperationRecord.operation_json`
        // is a `String` by contract); parse it here so it lands as native `jsonb`.
        let operation: serde_json::Value = serde_json::from_str(&record.operation_json)?;
        sql::append(
            &self.pool,
            record.did.as_str(),
            &record.cid,
            &record.op_type,
            record.prev.as_deref(),
            &operation,
            Utc::now(),
        )
        .await?;
        Ok(())
    }

    /// The `cid` of the DID's highest-`seq` (most recent) operation, or `None`.
    async fn latest_cid(&self, did: &Did) -> anyhow::Result<Option<String>> {
        Ok(sql::latest_cid(&self.pool, did.as_str()).await?)
    }

    /// The DID's most recent operation as a full record, or `None`. `operation`
    /// is re-serialized from `jsonb` to the JSON text the record carries.
    async fn latest_op(&self, did: &Did) -> anyhow::Result<Option<PlcOperationRecord>> {
        let row = sql::latest_op(&self.pool, did.as_str()).await?;

        Ok(row.map(|row| PlcOperationRecord {
            did: did.clone(),
            cid: row.cid,
            op_type: row.op_type,
            prev: row.prev,
            operation_json: row.operation.to_string(),
        }))
    }
}
