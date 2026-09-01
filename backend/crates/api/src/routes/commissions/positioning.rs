//! Account positioning endpoints (Ownership Separation DD `29130754`): a
//! commission is placed onto an account's board, and its view grants are
//! managed (`/placements`, `/grants`).
//!
//! **Placement is a card on a board** (Decision 6, "placement = workflow
//! membership rows, account-side"), so `/placements` addresses a **column** —
//! the column names its board, and the board names its account, so there is
//! nothing else to say. The account-level claim this endpoint used to make had
//! no referent in the corpus: it reconstructed the managing-account log the DD
//! superseded.

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
///
/// Deliberately **no** `account_id`: the column already determines its board and
/// the board its account, so an account field would be either redundant or a
/// second, contradictable source of truth.
#[derive(Deserialize)]
pub(super) struct PlaceBody {
    column_id: String,
    index: usize,
}

/// The `POST /commissions/{id}/grants` body: the target user and the key's
/// level (`presentation` / `description` / `total`). Grants are issued to a
/// User, never an Account (DD `29130754`, amended 2026-09-04).
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
/// one index. The caller must be a member of the board's account and must be
/// able to see the commission through either rail (publicly visible, or their
/// own standing or own key). Appends no changelog entry: positioning is
/// account-side view state the commission never learns about. Returns `204 No
/// Content`.
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
    // TODO(engineer): the `{account_id}` path segment predates the 2026-09-04
    // amendment to DD 29130754 (grants are per-User, never per-Account) and is
    // now dead — the target rides in the body. Deciding whether the segment
    // becomes the target user's DID, or the route drops it, is a contract call.
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
