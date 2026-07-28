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

/// `GET /health`'s body: liveness/readiness, `status` and `database` each one of
/// a fixed pair of tokens (`"ok"`/`"degraded"`, `"up"`/`"down"`) — see [`health`].
#[derive(Serialize)]
struct HealthBody {
    status: &'static str,
    database: &'static str,
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
async fn health(state: State<AppState>) -> (StatusCode, Json<HealthBody>) {
    if adapter_pg::is_reachable(&state.pool).await {
        (
            StatusCode::OK,
            Json(HealthBody {
                status: "ok",
                database: "up",
            }),
        )
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthBody {
                status: "degraded",
                database: "down",
            }),
        )
    }
}

#[cfg(test)]
mod tests {
    //! Pins the body's wire shape to the exact strings the retired
    //! `json!({ "status": …, "database": … })` literals produced (ZMVP-158
    //! AC1/AC3).

    use super::*;

    #[test]
    fn health_body_serializes_the_ok_pair() {
        let body = HealthBody {
            status: "ok",
            database: "up",
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"status":"ok","database":"up"}"#
        );
    }

    #[test]
    fn health_body_serializes_the_degraded_pair() {
        let body = HealthBody {
            status: "degraded",
            database: "down",
        };
        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            r#"{"status":"degraded","database":"down"}"#
        );
    }
}
