//! The health route group.
//!
//! `GET /health` is the one endpoint that intentionally fails when a dependency
//! is down, so an orchestrator can gate traffic. It carries no auth, changes no
//! state, and bears no cookie — so [`crate::app`] mounts it top-level, deliberately
//! *outside* the cookie-surface CSRF layer rather than under it.

use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;

use crate::AppState;

/// The health route group: just `GET /health`. Kept as its own builder so the
/// composition root can mount it top-level, alongside (not under) the
/// cookie-surface CSRF layer — `/health` must answer even a probe that carries
/// no `Origin` and no session.
pub(crate) fn health_router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

/// `GET /health`'s body: liveness/readiness, `database` and `status` each one of
/// a fixed pair of tokens (`"up"`/`"down"`, `"ok"`/`"degraded"`) — see [`health`].
///
/// Fields are declared alphabetically (`database` before `status`) to match the
/// key order the retired `json!({ "status": …, "database": … })` literal emitted
/// — `serde_json`'s `Map` is a `BTreeMap` here (no `preserve_order` feature), so
/// `json!` always serialized alphabetically regardless of literal order.
#[derive(Serialize)]
struct HealthResponse {
    database: &'static str,
    status: &'static str,
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
/// → 200 { "database": "up",   "status": "ok"       }
/// → 503 { "database": "down", "status": "degraded" }
/// ```
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
mod tests {
    //! Pins the body's wire shape to the exact strings the retired
    //! `json!({ "status": …, "database": … })` literals produced (ZMVP-158
    //! AC1/AC3) — alphabetical key order, matching `serde_json`'s `BTreeMap`
    //! (no `preserve_order`).

    use super::*;

    #[test]
    fn health_response_serializes_the_ok_pair() {
        let body = HealthResponse {
            database: "up",
            status: "ok",
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"database":"up","status":"ok"}"#
        );
    }

    #[test]
    fn health_response_serializes_the_degraded_pair() {
        let body = HealthResponse {
            database: "down",
            status: "degraded",
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"database":"down","status":"degraded"}"#
        );
    }
}
