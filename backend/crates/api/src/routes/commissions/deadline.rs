//! The deadline axis: `PUT`/`DELETE /commissions/{id}/deadline` (the deadline
//! envelope field) and `PUT`/`DELETE /commissions/{id}/status/deadline` (the
//! manual Delayed flag). `Late` is system-derived and never accepted here.

use application::commission::deadline;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::commission::{CommissionId, DeadlineStatus};
use serde::Deserialize;

use crate::{AppState, extract::CallingUser, problem::Problem};

/// The `PUT /commissions/{id}/deadline` request body: the new deadline as an
/// RFC 3339 timestamp. Clearing is `DELETE`, not a null body.
#[derive(Deserialize)]
pub(super) struct SetDeadlineBody {
    deadline: crate::wire_time::WireTimestamp,
}

/// The `PUT /commissions/{id}/status/deadline` request body: the deadline-axis
/// token to set. Only `delayed` — the manual slipping flag — is a Participant's
/// to set; `late` is refused (the system's word). Clearing is `DELETE`.
#[derive(Deserialize)]
pub(super) struct SetDeadlineStatusBody {
    status: String,
}

/// Sets (or moves) the commission's deadline. Any-Participant-gated;
/// idempotent on an unchanged value. Records `deadline_set` or
/// `deadline_extended`. Returns `204 No Content`.
pub(super) async fn set_deadline(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<SetDeadlineBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let deadline = body
        .deadline
        .try_into()
        .map_err(|err| Problem::invalid_request(format!("Invalid deadline: {err}.")))?;

    let command = deadline::set::Command {
        actor_id,
        commission_id,
        deadline,
    };

    state
        .app()
        .commissions()
        .deadline()
        .set(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Clears the commission's deadline, wiping the deadline axis with it.
/// Any-Participant-gated and idempotent. Returns `204 No Content`.
pub(super) async fn clear_deadline(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    let command = deadline::clear::Command {
        actor_id,
        commission_id,
    };

    state
        .app()
        .commissions()
        .deadline()
        .clear(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Sets the manual Delayed status. Any-Participant-gated; only `delayed` is
/// accepted (`late` is system-only, `422`). `409` with no deadline or if
/// already Late. Idempotent. Returns `204 No Content`.
pub(super) async fn set_deadline_status(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<SetDeadlineStatusBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    // The vocabulary gate is the wire's; `late` parses here and the use case
    // refuses it as the system's word alone.
    let status = body.status.parse::<DeadlineStatus>().map_err(|_| {
        Problem::invalid_request(format!(
            "{:?} is not a deadline status; expected: delayed.",
            body.status,
        ))
    })?;

    let command = deadline::status::set::Command {
        actor_id,
        commission_id,
        status,
    };

    state
        .app()
        .commissions()
        .deadline()
        .status()
        .set(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Clears the manual Delayed flag. Any-Participant-gated and idempotent; a
/// standing Late is a `409` (`commission_late`) — extend or clear the
/// deadline instead. Returns `204 No Content`.
pub(super) async fn clear_deadline_status(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    let command = deadline::status::clear::Command {
        actor_id,
        commission_id,
    };

    state
        .app()
        .commissions()
        .deadline()
        .status()
        .clear(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
