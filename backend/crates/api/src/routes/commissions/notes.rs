//! `POST /commissions/{id}/notes` — a Participant writes a free-text note into
//! the changelog stream (ZMVP-87 AC2; Changelog DD Decision 1). Speech into the
//! record, never dialogue: a note is a standalone entry with **no reply
//! affordances** — it cannot reference another entry, by shape.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{CommissionId, NewChangelogEntry},
    ports::transaction,
    string_builder::StringBuilder,
};
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem};

/// The `POST /commissions/{id}/notes` request body: just the free text. There is
/// deliberately no reply-to, thread, or mention field — the changelog is not a
/// chat (Changelog DD "What it is NOT").
#[derive(Deserialize)]
pub(super) struct WriteNoteBody {
    note: String,
}

/// Append a standalone note entry to the commission's stream (ZMVP-87 AC2).
///
/// Participant-only behind [`require_participant`](super::require_participant)
/// (uniform 404 for everyone else — the closed door). The text is trimmed and
/// must be non-empty (`422` otherwise) via [`StringBuilder`] (ZMVP-113); it lands as a
/// [`Note`](domain::elements::commission::ChangelogEntryKind::Note) entry in the
/// **same stream** as every domain event, appended through the unit of work like
/// any other entry. Returns `201 Created`.
pub(super) async fn write_note(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<WriteNoteBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let text = StringBuilder::new(body.note)
        .trimmed()
        .non_empty()
        .build()
        .map_err(|_| Problem::invalid_request("A note must not be empty."))?;

    let entry = NewChangelogEntry::note(commission, user.id, text, Utc::now());
    transaction(&*state.database, |uow| {
        Box::pin(async move { uow.changelog().append(&entry).await })
    })
    .await?;

    Ok(StatusCode::CREATED.into_response())
}
