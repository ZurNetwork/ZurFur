//! `POST /commissions/{id}/files/{file_id}/markup` — a Participant attaches a
//! Markup to a file entry (ZMVP-90; DESIGN/Commission — "File entries and
//! Markup"; Engineer ruling E14 2026-07-05).
//!
//! Markup is stored RAW and parsed by the frontend: this route validates
//! **strictly** (typed shapes, normalized 0–1 coordinates, capped text — see
//! [`Markup`]), then the markup rides the `markup_added` changelog entry's jsonb
//! payload exactly as submitted, referencing the validated file entry's id. The
//! changelog is append-only, so this boundary is the only gate the data will
//! ever pass — and also why markup has no edit or delete (immutability is
//! settled by ZMVP-87's shape, not a choice made here).
//!
//! **No Status side effects (the always-explicit rule; ZMVP-85/89 rulings).**
//! Adding markup never moves the Lifecycle, the direction axis, or the deadline
//! axis — the request body is `deny_unknown_fields`, so nothing status-shaped
//! can even ride along; the submission prompt (two explicit calls) is the path.
//!
//! Threading, persistence on file replacement, the annotate-matrix, and
//! retention are deferred to the File Activity & Markup 1DD.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{ChangelogEntryKind, CommissionId, FileKey, Markup, NewChangelogEntry},
    ports::transaction,
};
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem};

/// Attach a Markup to a file entry (ZMVP-90).
///
/// Any-Participant-gated behind
/// [`require_participant`](super::require_participant) (uniform 404 for everyone
/// else — the closed door). The file entry must exist **within this commission**
/// ([`find_file`](domain::ports::CommissionStore::find_file) is
/// commission-scoped): an unknown id — including another commission's — is
/// [`file_not_found`](Problem::file_not_found), never a cross-commission oracle.
///
/// The body is one [`Markup`], rejected `422` on any unknown shape/field,
/// out-of-range coordinate, malformed stroke, or bad text (the strict gate of an
/// append-only record; the serde/validation message is surfaced in `detail` so a
/// client can fix its canvas). What passes lands as a `markup_added` entry —
/// payload `{ file_id, markup }`, the markup **exactly as submitted** — through
/// the unit of work. The entry is the only write: no status, no lifecycle, no
/// deadline. Returns `201 Created`.
pub(super) async fn add_markup(
    State(state): State<AppState>,
    Path((id, file_id)): Path<(Uuid, Uuid)>,
    session: Session,
    body: Result<Json<Markup>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let key = FileKey::new(file_id);
    // Scoped to the commission: a key from another commission answers None here.
    state
        .commissions
        .find_file(commission, key)
        .await?
        .ok_or_else(Problem::file_not_found)?;

    let Json(markup) = body.map_err(|rejection| Problem::invalid_request(rejection.body_text()))?;
    markup
        .validate()
        .map_err(|e| Problem::invalid_request(format!("Invalid markup: {e}.")))?;

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::MarkupAdded,
        user.id,
        json!({
            "file_id": *key,
            "markup": markup,
        }),
        Utc::now(),
    );
    transaction(&*state.database, |uow| {
        Box::pin(async move { uow.changelog().append(&entry).await })
    })
    .await?;

    Ok(StatusCode::CREATED.into_response())
}
