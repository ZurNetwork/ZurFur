//! `POST /commissions/{id}/notes` — a Participant writes a free-text note into
//! the changelog stream. Standalone; no reply affordances.

use application::commission::notes::attach;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::commission::CommissionId;
use serde::Deserialize;

use crate::{AppState, extract::CallingUser, problem::Problem};

/// The `POST /commissions/{id}/notes` request body: just the free text.
#[derive(Deserialize)]
pub(super) struct WriteNoteBody {
    note: String,
}

/// Appends a standalone note entry to the commission's stream.
/// Participant-only; `422` for an empty note. Returns `201 Created`.
pub(super) async fn write_note(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<WriteNoteBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let command = attach::Command {
        commission_id,
        content: body.note,
        actor_id,
    };

    state
        .app()
        .commissions()
        .notes()
        .attach(command, Utc::now())
        .await?;

    Ok(StatusCode::CREATED.into_response())
}
