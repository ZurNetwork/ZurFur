//! `zurfur migrate` (ZMVP-206): apply the embedded migrations. The one
//! command exempt from the schema-drift gate — it is the fix the gate points
//! at. Idempotent: a current database reports `applied: 0`.

use composition::Runtime;
use serde_json::json;

use crate::CliError;

/// Apply pending migrations; `{"applied":N,"version":<latest embedded>}`.
pub async fn run(runtime: &Runtime) -> Result<serde_json::Value, CliError> {
    let report = adapter_pg::migrate_reporting(&runtime.pool)
        .await
        .map_err(|e| CliError::infra("service_unavailable", format!("migration failed: {e}")))?;
    let body = json!({
        "applied": report.applied,
        "version": report.version,
    });
    Ok(body)
}
