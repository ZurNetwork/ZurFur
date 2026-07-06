//! `GET /commissions/{id}/changelog` — a Participant reads the commission's
//! stream in order (ZMVP-87 AC5). Read-only by design: the changelog's HTTP
//! surface has no other method (append happens as a side of domain acts; AC4).

use axum::{
    Json,
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use domain::{datetime::DateTimeUtc, elements::commission::CommissionId};
use serde::Serialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem};

/// One changelog entry as the API serves it — the stored envelope, with the
/// kind as its stable token and the actor as a bare id (`null` = a system
/// entry). `seq` is the explicit ordering key (ascending = stream order);
/// `created_at` is carried for display.
#[derive(Serialize)]
struct ChangelogEntryBody {
    seq: i64,
    kind: &'static str,
    actor_id: Option<Uuid>,
    payload: serde_json::Value,
    note: Option<String>,
    created_at: DateTimeUtc,
}

/// Read the commission's changelog, in stream order (ZMVP-87 AC5): a bare JSON
/// array of entries, ascending `seq`. Participant-only behind
/// [`require_participant`](super::require_participant) — a non-participant (or
/// an absent commission) gets the uniform `commission_not_found` 404, never a
/// 403. Unpaginated at this ticket; cursors are ZMVP-100's job.
pub(super) async fn read_changelog(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let entries: Vec<ChangelogEntryBody> = state
        .changelog
        .entries(commission)
        .await?
        .into_iter()
        .map(|entry| ChangelogEntryBody {
            seq: entry.seq,
            kind: entry.kind.as_str(),
            actor_id: entry.actor_id.map(|actor| *actor),
            payload: entry.payload,
            note: entry.note,
            created_at: entry.created_at,
        })
        .collect();

    Ok(Json(entries).into_response())
}
