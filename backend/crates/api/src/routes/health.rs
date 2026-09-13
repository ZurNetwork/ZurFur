//! The health route group: `GET /health`, the one endpoint that intentionally
//! fails when a dependency is down. No auth, no cookie; mounted outside the
//! CSRF layer.

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;

use crate::AppState;

/// The health route group: just `GET /health`, mounted top-level alongside
/// (not under) the cookie-surface CSRF layer.
pub(crate) fn health_router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

/// `GET /health`'s body — see [`health`]. Fields are declared alphabetically
/// (`database` before `status`) to match `serde_json`'s `BTreeMap` key order.
#[derive(Serialize)]
struct HealthResponse {
    database: &'static str,
    status: &'static str,
}

/// `GET /health` — `200` when Postgres is reachable, `503 degraded` when not.
/// Only the database is probed; a `200` doesn't certify the PDS or any other adapter.
async fn health(state: State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    if adapter_pg::is_reachable(&state.pool).await {
        (
            StatusCode::OK,
            Json(HealthResponse {
                database: "up",
                status: "ok",
            }),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthResponse {
                database: "down",
                status: "degraded",
            }),
        )
    }
}

#[cfg(test)]
mod tests;
