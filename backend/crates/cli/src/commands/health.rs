//! `zurfur health` (ZMVP-202): the same probe `GET /health` runs —
//! [`adapter_pg::is_reachable`], defined once in the adapter and consumed by
//! both drivers — plus the round-trip latency. Booting the runtime already
//! proved a connection could be opened; this asks the pool to answer.

use std::time::Instant;

use composition::Runtime;
use serde_json::json;

use crate::CliError;

/// Probe the pool. `{"status":"ok","database":"up","latency_ms":N}` on
/// success; otherwise a `service_unavailable` infrastructure problem (the
/// API's own code for a down dependency — one vocabulary, Engineer ruling
/// 2026-08-24) whose `detail` says the pool exists but the database did not
/// answer in time, as opposed to the runtime's connect failure.
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
    let report = json!({
        "status": "ok",
        "database": "up",
        "latency_ms": latency_ms,
    });
    Ok(report)
}
