//! Account positioning endpoints: a commission is placed onto
//! an account's board, and its view grants are managed (`/placements`,
//! `/grants`). Placement addresses a column — the column names its board,
//! the board its account — so `/placements` carries no `account_id`.

use application::{account, commission::view};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::{
    account::AccountId,
    commission::{CommissionId, GrantLevel},
    user::UserId,
    workflow::ColumnId,
};
use serde::Deserialize;

use crate::{AppState, extract::CallingUser, problem::Problem};

/// The `POST /commissions/{id}/placements` body: the column to place the
/// commission in, and where in it.
#[derive(Deserialize)]
pub(super) struct PlaceBody {
    column_id: String,
    index: usize,
}

/// The `POST /commissions/{id}/grants` body: the target user and the key's
/// level (`presentation`/`description`/`total`). Grants are issued to Users,
/// never to Accounts.
#[derive(Deserialize)]
pub(super) struct GrantBody {
    target_user_id: String,
    level: String,
}

/// The `DELETE /commissions/{id}/grants/{account_id}` body: the user whose
/// key to revoke.
#[derive(Deserialize)]
pub(super) struct RevokeBody {
    pub target_user_id: String,
}

/// Places the commission on an account's board — one card, in one column, at
/// one index. Requires board membership and commission visibility. Appends no
/// changelog entry: positioning is account-side view state. `204 No Content`.
pub(super) async fn place_commission(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<PlaceBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let column_id = body
        .column_id
        .parse::<ColumnId>()
        .map_err(|_| Problem::invalid_request("The column must be a UUID."))?;

    let index = body.index;
    let command = account::workflow::column::commission::set_in_column::Command {
        column_id,
        index,
        actor_id,
        commission_id,
    };

    state.app().commissions().insert_in_column(command).await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Issues a User a view grant at an explicit level (`presentation` /
/// `description` / `total`). Owner-only; `422` for an unrecognized level.
/// Re-granting replaces the level. Returns `204 No Content`.
pub(super) async fn grant_view(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<GrantBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let level = body.level.parse::<GrantLevel>().map_err(|_| {
        Problem::invalid_request(format!(
            "{:?} is not a grant level; expected one of: presentation, description, total.",
            body.level,
        ))
    })?;
    let target_user_id = body
        .target_user_id
        .parse::<UserId>()
        .map_err(|_| Problem::invalid_request("The user must be a DID, e.g. \"did:plc:…\"."))?;

    let command = view::grant::Command {
        actor_id,
        commission_id,
        level,
        target_user_id,
    };

    state
        .app()
        .commissions()
        .view()
        .grant(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Revokes a User's view grant, hard-deleting the key. Owner-only and
/// idempotent — revoking a user with no key is a no-op. Returns `204 No
/// Content`.
pub(super) async fn revoke_view(
    State(state): State<AppState>,
    // TODO(engineer): `{account_id}` is dead since grants went per-User (DD 29130754).
    Path((commission_id, _account_id)): Path<(CommissionId, AccountId)>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<RevokeBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let target_user_id = body
        .target_user_id
        .parse::<UserId>()
        .map_err(|_| Problem::invalid_request("The user must be a DID, e.g. \"did:plc:…\"."))?;

    let command = view::revoke::Command {
        actor_id,
        commission_id,
        target_user_id,
    };

    state
        .app()
        .commissions()
        .view()
        .revoke(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
