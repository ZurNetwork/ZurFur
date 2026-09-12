//! `POST /commissions/{id}/files/{file_id}/markup` — a Participant attaches a
//! Markup to a file entry. Routing only (ZMVP-205): the use case lives in
//! `application::commission::markup`; this file decodes the request and renders
//! the response.
//!
//! The strict shape gate is `serde`'s, so it fires **here**, at the wire: an
//! unknown shape kind, an unknown field, or a missing coordinate never becomes a
//! [`Markup`] at all. The numeric bounds are the use case's.

use application::commission::markup;
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::commission::{CommissionId, FileKey, Markup};

use crate::{AppState, extract::CallingUser, problem::Problem};

/// Attaches a Markup to a file entry. Any-Participant-gated; `404
/// file_not_found` for a file not in this commission, `422` for an invalid
/// markup shape. Lands as a `commission_markup` row plus the `markup_added`
/// changelog entry that accompanies it. Returns `201 Created`.
///
/// ⚠️ contract-decision-needed: `Markup` is an unschematized passthrough
/// (tracks `VERSIONING.md` §8 Q9).
pub(super) async fn add_markup(
    State(state): State<AppState>,
    Path((commission_id, file_key)): Path<(CommissionId, FileKey)>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<Markup>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(markup) = body.map_err(|rejection| Problem::invalid_request(rejection.body_text()))?;

    let command = markup::Command {
        actor_id,
        commission_id,
        file_key,
        markup,
    };

    state
        .app()
        .commissions()
        .markup(command, Utc::now())
        .await?;

    Ok(StatusCode::CREATED.into_response())
}
