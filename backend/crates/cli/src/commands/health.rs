//! `zurfur health`: the same probe `GET /health` runs —
//! [`adapter_pg::is_reachable`], consumed by both drivers — plus round-trip
//! latency. Booting the runtime already proved a connection could open;
//! this asks the pool to answer.

use std::time::Instant;

use composition::Runtime;
use serde_json::json;

use crate::CliError;

/// Probe the pool. `{"status":"ok","database":"up","schema":"current"|
/// "behind"|"ahead"|"unknown","latency_ms":N}` on success — `health` reports
/// the schema state where every other command refuses it; otherwise a
/// `service_unavailable` infrastructure problem.
pub async fn run(runtime: &Runtime) -> Result<serde_json::Value, CliError> {
    let started = Instant::now();
    let reachable = adapter_pg::is_reachable(&runtime.pool).await;
    let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if !reachable {
        return Err(CliError::infra(
            "service_unavailable",
            "the database did not answer the health query within its timeout",
        ));
    }
    let schema = match adapter_pg::schema_status(&runtime.pool).await {
        Ok(adapter_pg::SchemaStatus::Current) => "current",
        Ok(adapter_pg::SchemaStatus::Behind { .. }) => "behind",
        Ok(adapter_pg::SchemaStatus::Ahead { .. }) => "ahead",
        Ok(adapter_pg::SchemaStatus::Unknown) => "unknown",
        Err(error) => {
            // health describes rather than refuses; the cause still goes to stderr.
            tracing::warn!(%error, "schema status could not be read");
            "unknown"
        }
    };
    let report = json!({
        "status": "ok",
        "database": "up",
        "schema": schema,
        "latency_ms": latency_ms,
    });
    Ok(report)
}
