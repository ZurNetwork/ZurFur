//! `POST /commissions/{id}/archive` / `POST /commissions/{id}/unarchive` — the
//! owner archives, or un-archives, a commission (the soft-delete path, DD
//! `3014657`).

use application::commission::{archive, unarchive};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::commission::CommissionId;

use crate::{AppState, extract::CallingUser, problem::Problem};

/// Archives the commission: owner-only, leaves the active views while the
/// record survives. Idempotent — archiving an already-archived commission is
/// a no-op. Returns `204 No Content`.
pub(super) async fn archive_commission(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    let command = archive::Command {
        actor_id,
        commission_id,
    };

    state
        .app()
        .commissions()
        .archive(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Un-archives the commission, returning it to active views. Owner-only and
/// idempotent, mirroring [`archive_commission`]. Returns `204 No Content`.
pub(super) async fn unarchive_commission(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    let command = unarchive::Command {
        actor_id,
        commission_id,
    };

    state
        .app()
        .commissions()
        .unarchive(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
