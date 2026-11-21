//! `PUT /commissions/{id}/lifecycle` — a Participant moves the commission's lifecycle step.

use application::commission::lifecycle::move_to;
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::commission::LifecycleStep;

use crate::generated::MoveCommissionLifecycleRequest;
use crate::{
    AppState,
    extract::{CallingUser, CommissionPath},
    problem::{FieldCode, FieldPointer, FieldRefusal, Problem},
};

/// `422 invalid_request` with one field error, at `#/to`: the step isn't one the vocabulary
/// names. Its detail is fixed, so it never echoes the token sent.
fn unknown_step() -> Problem {
    let step_refusal = FieldRefusal {
        pointer: FieldPointer::member("to"),
        code: FieldCode::UnknownValue,
    };
    Problem::invalid_fields(step_refusal, [])
}

/// Moves the commission the path names to the step the body names; an `id` in the body is
/// ignored.
///
/// `204` once moved, or with nothing changed when it already holds the step; `422` for a
/// malformed body, or with a field error at `#/to` for a token outside the vocabulary, checked
/// before anyone's standing; `404` for anyone who holds no role on it and for an id that names
/// nothing; `409` for a move its rule refuses.
pub(super) async fn move_lifecycle(
    State(state): State<AppState>,
    CallingUser(actor_id): CallingUser,
    CommissionPath(commission_id): CommissionPath,
    body: Result<Json<MoveCommissionLifecycleRequest>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let to = body
        .to
        .parse::<LifecycleStep>()
        .map_err(|_| unknown_step())?;

    let command = move_to::Command {
        actor_id,
        commission_id,
        to,
    };
    state
        .app()
        .commissions()
        .lifecycle()
        .move_to(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
