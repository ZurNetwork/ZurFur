//! `PUT`/`DELETE /commissions/{id}/dispute` — a Participant raises a dispute on the
//! commission, or clears it back to the step it left.

use application::commission::dispute::{clear, raise};
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;

use crate::{
    AppState,
    extract::{CallingUser, CommissionPath},
    problem::Problem,
};

/// Raises a dispute on the commission the path names, from whatever step it holds; a body is
/// ignored.
///
/// `204` once raised; `404` for anyone who holds no role on it and for an id that names
/// nothing; `409 commission_disputed` while it is already disputed.
pub(super) async fn raise_dispute(
    State(state): State<AppState>,
    CallingUser(actor_id): CallingUser,
    CommissionPath(commission_id): CommissionPath,
) -> Result<Response, Problem> {
    let command = raise::Command {
        actor_id,
        commission_id,
    };
    state
        .app()
        .commissions()
        .dispute()
        .raise(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Clears the dispute on the commission the path names, returning it to the step it left.
///
/// `204` once cleared; `404` for anyone who holds no role on it and for an id that names
/// nothing; `409 commission_not_disputed` when it isn't disputed.
pub(super) async fn clear_dispute(
    State(state): State<AppState>,
    CallingUser(actor_id): CallingUser,
    CommissionPath(commission_id): CommissionPath,
) -> Result<Response, Problem> {
    let command = clear::Command {
        actor_id,
        commission_id,
    };
    state
        .app()
        .commissions()
        .dispute()
        .clear(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
