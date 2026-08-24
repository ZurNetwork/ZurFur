//! `zurfur health` (ZMVP-202): the same probe `GET /health` runs —
//! [`adapter_pg::is_reachable`], defined once in the adapter and consumed by
//! both drivers — plus the round-trip latency. Booting the runtime already
//! proved a connection could be opened; this asks the pool to answer.

use std::time::Instant;

use composition::Runtime;
use serde_json::json;

use crate::CliError;

/// Probe the pool. `{"status":"ok","database":"up","latency_ms":N}` on
/// success; a `database_unreachable` infrastructure problem otherwise.
pub async fn run(runtime: &Runtime) -> Result<serde_json::Value, CliError> {
    let started = Instant::now();
    let reachable = adapter_pg::is_reachable(&runtime.pool).await;
    let latency_ms = started.elapsed().as_millis();
    if !reachable {
        return Err(CliError::infra(
            "database_unreachable",
            "the database did not answer the health query within its timeout",
        ));
    }
    let report = json!({
        "status": "ok",
        "database": "up",
        "latency_ms": latency_ms,
    });
    Ok(report)
}
