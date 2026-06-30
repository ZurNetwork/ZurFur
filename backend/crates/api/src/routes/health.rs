//! The health route group.
//!
//! `GET /health` is the one endpoint that intentionally fails when a dependency
//! is down, so an orchestrator can gate traffic. It carries no auth, changes no
//! state, and bears no cookie — so [`crate::app`] mounts it top-level, deliberately
//! *outside* the cookie-surface CSRF layer rather than under it.

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde_json::json;

use crate::AppState;

/// The health route group: just `GET /health`. Kept as its own builder so the
/// composition root can mount it top-level, alongside (not under) the
/// cookie-surface CSRF layer — `/health` must answer even a probe that carries
/// no `Origin` and no session.
pub(crate) fn health_router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

/// Liveness/readiness probe (`GET /health`). Reports `200` with the database
/// `up` when the pool can reach Postgres, `503 degraded` when it can't — the one
/// endpoint that intentionally fails when a dependency is down, so an
/// orchestrator can gate traffic. No auth.
///
/// Caveats: only the database is probed; a healthy `200` doesn't certify the PDS
/// or any other adapter. References: CLAUDE.md "Database"; [`adapter_pg::is_reachable`].
///
/// ```text
/// GET /health
/// → 200 { "status": "ok",       "database": "up"   }
/// → 503 { "status": "degraded", "database": "down" }
/// ```
async fn health(state: State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    if adapter_pg::is_reachable(&state.pool).await {
        (
            StatusCode::OK,
            Json(json!({ "status": "ok", "database": "up" })),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "status": "degraded", "database": "down" })),
        )
    }
}
