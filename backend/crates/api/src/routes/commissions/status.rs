//! `PUT`/`DELETE /commissions/{id}/status/direction` — a Participant sets or
//! clears the commission's **direction-axis Status** (ZMVP-85;
//! DESIGN/Commission, Status). Direction transitions are ALWAYS an explicit
//! Participant act (Engineer ruling 2026-07-01): no content event or system
//! sweep moves this axis — this endpoint is the column's only writer. One
//! nullable slot (ruling E29), so a set REPLACES the current value and axis
//! exclusivity holds by construction; the deadline axis (ZMVP-86) is separate
//! and the two compose freely. Every change is changelog-recorded, atomically.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::{
        commission::{ChangelogEntryKind, CommissionId, DirectionStatus, NewChangelogEntry},
        user::UserId,
    },
    ports::transaction,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem};

/// The `PUT /commissions/{id}/status/direction` request body: the direction
/// value's stable wire token (`waiting_for_input` / `waiting_for_approval` /
/// `changes_requested`). Clearing is `DELETE`, not a null body.
#[derive(Deserialize)]
pub(super) struct SetDirectionStatusBody {
    status: String,
}

/// Set (or replace) the commission's direction status (ZMVP-85 AC1/AC2).
///
/// Any-Participant-gated behind
/// [`require_participant`](super::require_participant) (uniform 404 for
/// everyone else — the closed door). A token outside the three-value
/// vocabulary ([`DirectionStatus`]'s `TryFrom<&str>`) is a `422`. Setting the value
/// already held changes nothing: `204` with **no** entry appended (a record of
/// nothing changing would be noise, not audit — the linked-channel precedent).
/// Otherwise the column write and the `status_changed` changelog entry
/// (payload carries `from`/`to` tokens, so it renders without joins) land in
/// **one unit of work** (Changelog DD D4). Returns `204 No Content`.
pub(super) async fn set_direction_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<SetDirectionStatusBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let status = DirectionStatus::try_from(body.status.as_str()).map_err(|_| {
        Problem::invalid_request(format!(
            "Unknown direction status {:?}; expected one of: {}.",
            body.status,
            DirectionStatus::ALL
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        ))
    })?;

    apply_direction_status(&state, commission, user.id, Some(status)).await
}

/// Clear the commission's direction status (ZMVP-85 AC1).
///
/// Any-Participant-gated like the set. Clearing an already-clear status is an
/// idempotent no-op — `204` with no entry; otherwise the column goes `NULL`
/// and the `status_changed` entry (`to: null`) lands in the same unit of work.
/// Returns `204 No Content`.
pub(super) async fn clear_direction_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    apply_direction_status(&state, commission, user.id, None).await
}

/// The shared set/clear tail: read the current value, drop a no-op early
/// (nothing changed — no entry), otherwise write the one nullable column and
/// append the `status_changed` entry **in one unit of work**.
///
/// The participant gate has already passed, so a vanished commission here (a
/// delete racing this request) surfaces as the same uniform
/// [`commission_not_found`](Problem::commission_not_found).
async fn apply_direction_status(
    state: &AppState,
    commission: CommissionId,
    actor: UserId,
    to: Option<DirectionStatus>,
) -> Result<Response, Problem> {
    let found = state
        .commissions
        .find(commission)
        .await?
        .ok_or_else(Problem::commission_not_found)?;
    let from = found.direction_status;
    if from == to {
        return Ok(StatusCode::NO_CONTENT.into_response());
    }

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::StatusChanged,
        actor,
        json!({
            "from": from.map(|s| s.as_str()),
            "to": to.map(|s| s.as_str()),
        }),
        Utc::now(),
    );
    transaction(&*state.database, |uow| {
        Box::pin(async move {
            // Gate the changelog entry on the atomic `changed` flag the write
            // returns (`… IS DISTINCT FROM`), so a value that raced to `to`
            // between the read above and this write appends no spurious entry —
            // the linked-channel contract (PR #102 review).
            if uow
                .commissions()
                .set_direction_status(commission, to)
                .await?
            {
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
    })
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
