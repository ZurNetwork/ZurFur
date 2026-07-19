//! The deadline axis (ZMVP-86; DESIGN/Commission, Status) — the Participant's
//! half. Two surfaces:
//!
//! - `PUT`/`DELETE /commissions/{id}/deadline` — a Participant sets, extends,
//!   or clears the nullable-but-fixed deadline envelope field (AC1), with
//!   `deadline_set`/`deadline_extended` changelog entries.
//! - `PUT`/`DELETE /commissions/{id}/status/deadline` — a Participant sets or
//!   clears the manual **Delayed** "slipping" flag (Engineer ruling
//!   2026-07-05: Delayed is an explicit Participant act, never derived).
//!
//! **Late is the system's word**: only the deadline sweeper
//! ([`crate::sweep_deadlines`]) sets it, and no handler here accepts or erases
//! it — a Participant resolves Late through the deadline itself (extend or
//! clear). One nullable cell per axis (ruling E29), so exclusivity holds by
//! construction; the direction axis (ZMVP-85) is separate and the two compose
//! freely. A commission with no deadline never carries a deadline-axis status
//! (AC4): the flag is refused without a deadline, and clearing the deadline
//! wipes the axis. Every change is changelog-recorded, atomically.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, DeadlineStatus, NewChangelogEntry},
        user::UserId,
    },
    ports::UnitOfWork,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem};

/// The `PUT /commissions/{id}/deadline` request body: the new deadline as an
/// RFC 3339 timestamp. Clearing is `DELETE`, not a null body.
#[derive(Deserialize)]
pub(super) struct SetDeadlineBody {
    deadline: DateTimeUtc,
}

/// The `PUT /commissions/{id}/status/deadline` request body: the deadline-axis
/// token to set. Only `delayed` — the manual slipping flag — is a Participant's
/// to set; `late` is refused (the system's word). Clearing is `DELETE`.
#[derive(Deserialize)]
pub(super) struct SetDeadlineStatusBody {
    status: String,
}

/// Set (or move) the commission's deadline (ZMVP-86 AC1).
///
/// Any-Participant-gated behind
/// [`require_participant`](super::require_participant) (uniform 404 for
/// everyone else — the closed door). Setting the deadline already held changes
/// nothing: `204` with **no** entry appended (the no-op precedent). Otherwise
/// the envelope write and its changelog entry land in **one unit of work**
/// (Changelog DD D4): pushing an existing deadline later records as
/// `deadline_extended`, anything else (first set, or pulling it earlier) as
/// `deadline_set` — payload carries `from`/`to`, a sentence without joins. If
/// the commission stood **Late** and the new deadline hasn't passed, the
/// system's mark no longer holds and the axis clears in the same unit (a
/// standing manual Delayed survives — it is a human flag, cleared by a human).
/// Returns `204 No Content`.
pub(super) async fn set_deadline(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<SetDeadlineBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    apply_deadline(&state, commission, user.id, Some(body.deadline)).await
}

/// Clear the commission's deadline (ZMVP-86 AC1/AC4).
///
/// Any-Participant-gated like the set. Clearing an absent deadline is an
/// idempotent no-op — `204`, no entry. Otherwise the envelope goes `NULL`
/// **and the deadline axis is wiped with it** (a commission with no deadline
/// never carries deadline-axis statuses — this is also the Participant's
/// honest lever against a standing Late: no deadline, nothing to be late
/// against), with the `deadline_set` entry (`to: null`) in the same unit of
/// work. Returns `204 No Content`.
pub(super) async fn clear_deadline(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    apply_deadline(&state, commission, user.id, None).await
}

/// The shared deadline set/clear tail: read the current envelope to decide the
/// entry kind (`deadline_extended` only for a later `to` over a standing
/// `from`), then land the deadline write plus the entry **in one unit of
/// work** — appending only when the write actually changed the stored value
/// (the atomic `IS DISTINCT FROM`), so a re-set of the same deadline records
/// no entry (`204`) even under a concurrent racing write.
///
/// The deadline-axis status needs no touch here: **`Late` is derived on lookup**
/// from `deadline < now` (Engineer ruling 2026-07-08), so moving or clearing the
/// deadline simply changes when it derives true — there is no stored `Late` to
/// clear (AC4 falls out for free). A standing manual `Delayed` is a human flag
/// and is left untouched.
///
/// The participant gate has already passed, so a vanished commission here (a
/// delete racing this request) surfaces as the same uniform
/// [`commission_not_found`](Problem::commission_not_found).
async fn apply_deadline(
    state: &AppState,
    commission: CommissionId,
    actor: UserId,
    to: Option<DateTimeUtc>,
) -> Result<Response, Problem> {
    let found = state
        .commissions
        .find(commission)
        .await?
        .ok_or_else(Problem::commission_not_found)?;
    let from = found.deadline;

    let now = Utc::now();
    let kind = match (from, to) {
        (Some(old), Some(new)) if new > old => ChangelogEntryKind::DeadlineExtended,
        _ => ChangelogEntryKind::DeadlineSet,
    };
    let entry = NewChangelogEntry::event(
        commission,
        kind,
        actor,
        json!({ "from": from, "to": to }),
        now,
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            // Append only on a real change — the write's atomic `IS DISTINCT
            // FROM` result is the single arbiter, not the non-atomic pre-read
            // above (which only shapes the entry). A re-set of the same value
            // makes no entry, even under a concurrent racing write; mirrors
            // set_direction_status (ultrareview 2026-07-18).
            if uow.commissions().set_deadline(commission, to).await? {
                uow.changelog().append(&entry).await?;
            }
            anyhow::Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Flag the commission as slipping — set the manual **Delayed** status
/// (ZMVP-86; Engineer ruling 2026-07-05).
///
/// Any-Participant-gated behind
/// [`require_participant`](super::require_participant). The only token this
/// endpoint accepts is `delayed`: `late` is refused with a `422` (the system's
/// word, set only by the sweeper), as is anything outside the axis vocabulary.
/// A commission with no deadline is a `409` ([`Problem::no_deadline`] — AC4);
/// one already Late is a `409` ([`Problem::commission_late`] — the flag cannot
/// downgrade the system's mark). Re-flagging is an idempotent no-op (`204`,
/// no entry); otherwise the one-slot write and its `delayed` changelog entry
/// (an **actor-bearing event**, never a system entry) land in one unit of
/// work. Returns `204 No Content`.
pub(super) async fn set_deadline_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<SetDeadlineStatusBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    match DeadlineStatus::try_from(body.status.as_str()) {
        Ok(DeadlineStatus::Delayed) => {}
        Ok(DeadlineStatus::Late) => {
            return Err(Problem::invalid_request(
                "Late is set by the system when the deadline passes; it can't be set by hand.",
            ));
        }
        Err(_) => {
            return Err(Problem::invalid_request(format!(
                "Unknown deadline status {:?}; expected \"delayed\" (Late is system-set).",
                body.status,
            )));
        }
    }

    apply_deadline_status(&state, commission, user.id, Some(DeadlineStatus::Delayed)).await
}

/// Clear the manual Delayed flag (ZMVP-86).
///
/// Any-Participant-gated like the set. Clearing an already-clear axis is an
/// idempotent no-op — `204`, no entry. A standing **Late** is a `409`
/// ([`Problem::commission_late`]): the system's word is not erased by hand —
/// extend or clear the deadline instead. Otherwise the slot goes `NULL` and
/// the `delayed` entry (`to: null`) lands in the same unit of work. Returns
/// `204 No Content`.
pub(super) async fn clear_deadline_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    apply_deadline_status(&state, commission, user.id, None).await
}

/// The shared Delayed-flag set/clear tail: enforce the axis preconditions
/// (a deadline must exist to flag against — AC4; a standing Late is the
/// system's and conflicts either way), then write the one nullable cell and
/// append the `delayed` entry **in one unit of work** — the entry landing only
/// when the write actually changed the value (the atomic `IS DISTINCT FROM`),
/// so a re-flag records no entry (`204`).
async fn apply_deadline_status(
    state: &AppState,
    commission: CommissionId,
    actor: UserId,
    to: Option<DeadlineStatus>,
) -> Result<Response, Problem> {
    let found = state
        .commissions
        .find(commission)
        .await?
        .ok_or_else(Problem::commission_not_found)?;
    let from = found.deadline_status;
    // Real preconditions (business 409s), kept before the write: a standing
    // system Late is not the actor's to downgrade, and a Delayed flag needs a
    // deadline to flag against (AC4). The no-op case is NOT decided here — the
    // atomic write below is its single arbiter.
    if from == Some(DeadlineStatus::Late) {
        return Err(Problem::commission_late());
    }
    if to.is_some() && found.deadline.is_none() {
        return Err(Problem::no_deadline());
    }

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::Delayed,
        actor,
        json!({
            "from": from.map(|s| s.as_str()),
            "to": to.map(|s| s.as_str()),
            "deadline": found.deadline,
        }),
        Utc::now(),
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            // Append only on a real change (the write's atomic `IS DISTINCT
            // FROM`), never a spurious `delayed` entry on a re-set; mirrors
            // set_direction_status (ultrareview 2026-07-18).
            if uow
                .commissions()
                .set_deadline_status(commission, to)
                .await?
            {
                uow.changelog().append(&entry).await?;
            }
            anyhow::Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
