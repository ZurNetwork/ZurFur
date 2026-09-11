//! `PUT`/`DELETE /commissions/{id}/status/direction` — a Participant sets or
//! clears the commission's direction-axis Status. Always an explicit
//! Participant act; a set replaces the current value.

use application::commission::status::direction;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::commission::{CommissionId, DirectionStatus};
use serde::Deserialize;

use crate::{AppState, extract::CallingUser, problem::Problem};

/// The `PUT /commissions/{id}/status/direction` request body: the direction
/// value's stable wire token (`waiting_for_input` / `waiting_for_approval` /
/// `changes_requested`). Clearing is `DELETE`, not a null body.
#[derive(Deserialize)]
pub(super) struct SetDirectionStatusBody {
    status: String,
}

/// Sets (or replaces) the commission's direction status. Any-Participant-gated;
/// `422` for a token outside the vocabulary; idempotent on an unchanged
/// value. Returns `204 No Content`.
pub(super) async fn set_direction_status(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(user_id): CallingUser,
    body: Result<Json<SetDirectionStatusBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let status = DirectionStatus::try_from(body.status.as_str()).map_err(|_| {
        let vocabulary = DirectionStatus::ALL
            .iter()
            .map(|status| status.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        Problem::invalid_request(format!(
            "unknown direction status {:?}; expected one of: {vocabulary}.",
            body.status,
        ))
    })?;

    let command = direction::set::Command {
        direction: Some(status),
        commission_id,
        user_id,
    };

    state
        .app()
        .commissions()
        .status()
        .direction()
        .set(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Clears the commission's direction status. Any-Participant-gated and
/// idempotent. Returns `204 No Content`.
pub(super) async fn clear_direction_status(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(user_id): CallingUser,
) -> Result<Response, Problem> {
    let command = direction::clear::Command {
        commission_id,
        direction: None,
        user_id,
    };

    state
        .app()
        .commissions()
        .status()
        .direction()
        .clear(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
