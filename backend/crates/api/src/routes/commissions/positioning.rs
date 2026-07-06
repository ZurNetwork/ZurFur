//! Account **positioning** endpoints (ZMVP-70; Ownership Separation DD
//! `29130754`): the owner places a commission in an account's position and manages
//! that account's view grants. Two rails, kept apart exactly as the DD keeps them
//! (Decision 6):
//!
//! - `POST /commissions/{id}/placements` — append a placement-log row (and repoint
//!   the current-placement pointer). Placement is account-side positioning; it
//!   appends **no** changelog entry — the placement log *is* the record.
//! - `POST /commissions/{id}/grants` — issue an account a *key to see* at an
//!   explicit level; `DELETE /commissions/{id}/grants/{account_id}` — revoke it,
//!   hard-deleting the key (effective on the next serialization). Issue/revoke are
//!   recorded-but-not-broadcast changelog events (Decision 5).
//!
//! **Authority is owner-only in v1** via the shared [`require_owner`] seam — the
//! same seam that widens to Commission Admin when ZMVP-83 lands (view grants are
//! Admin-capable per Structural Authority `29425666`; that activation sweeps this
//! call site with the others). Neither rail confers any in-commission authority
//! (Decision 8): a key is only a view, positioning is environmental.
//!
//! **The closed door.** A non-owner who may not learn the commission exists gets
//! the uniform [`Problem::commission_not_found`] 404, never a 403 oracle
//! ([`require_owner`]).

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountId},
        commission::{ChangelogEntryKind, CommissionId, GrantLevel, NewChangelogEntry},
    },
    ports::transaction,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// The `POST /commissions/{id}/placements` body: the target account.
#[derive(Deserialize)]
pub(super) struct PlaceBody {
    account_id: String,
}

/// The `POST /commissions/{id}/grants` body: the target account and the key's
/// level (`presentation` / `description` / `total`).
#[derive(Deserialize)]
pub(super) struct GrantBody {
    account_id: String,
    level: String,
}

/// Parse a body-supplied account id and resolve it to a **live** [`Account`] — the
/// shared front half of place and grant. A malformed id is a `422`; an id that
/// resolves to nothing (absent or soft-deleted — `find` filters tombstones) is a
/// `404 account_not_found`. The caller has already passed [`require_owner`], so it
/// may honestly learn the *account* is unknown (that leaks nothing about the
/// commission's existence, which it already knows). Returns the whole account so a
/// caller that needs its handle (the grant's changelog sentence) does not re-fetch.
async fn resolve_live_account(state: &AppState, raw: &str) -> Result<Account, Problem> {
    let account = AccountId::new(
        Uuid::parse_str(raw).map_err(|_| Problem::invalid_request("Malformed account id."))?,
    );
    state
        .accounts
        .find(account)
        .await?
        .ok_or_else(Problem::account_not_found)
}

/// Place the commission in an account's position (ZMVP-70 AC1/AC2/AC3).
///
/// Owner-only ([`require_owner`]). Appends one placement-log row and repoints the
/// current-placement pointer to it, atomically, so the pointer equals the latest
/// row after every (re)placement. Re-placement always appends; the log is never
/// rewritten. **No changelog entry** is appended — placement is account-side
/// positioning and the log is its own record (the Changelog DD taxonomy carries
/// no placement variant). Returns `204 No Content`.
pub(super) async fn place_commission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<PlaceBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let account = resolve_live_account(&state, &body.account_id).await?.id;

    let now = Utc::now();
    transaction(&*state.database, |uow| {
        Box::pin(async move {
            uow.commissions()
                .place(commission, account, user.id, now)
                .await
        })
    })
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Issue an account a view grant at an explicit level (ZMVP-70 AC4).
///
/// Owner-only ([`require_owner`]). The level is parsed by [`GrantLevel::parse`]
/// — the raw modes `presentation`/`description`/`total`, never the Private/Listed/
/// Public aliases — a bad value is a `422`. The key upsert (re-granting replaces
/// the level) and the `view_grant_issued` changelog entry (payload names the
/// account handle and level, so it renders without joins) land in **one unit of
/// work**. Returns `204 No Content`.
pub(super) async fn grant_view(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<GrantBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let level = GrantLevel::parse(&body.level).ok_or_else(|| {
        Problem::invalid_request(
            "level must be one of: presentation, description, total.".to_string(),
        )
    })?;
    let account = resolve_live_account(&state, &body.account_id).await?;
    let account_id = account.id;

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::ViewGrantIssued,
        user.id,
        json!({
            "account_id": *account_id,
            "account_handle": account.handle.as_str(),
            "level": level.as_str(),
        }),
        Utc::now(),
    );
    transaction(&*state.database, |uow| {
        Box::pin(async move {
            uow.commissions()
                .grant_view(commission, account_id, level)
                .await?;
            uow.changelog().append(&entry).await
        })
    })
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Revoke an account's view grant (ZMVP-70 AC4; DD Decision 5).
///
/// Owner-only ([`require_owner`]). Hard-deletes the key — revocation is effective
/// on the next server-side serialization by construction (no session to
/// invalidate). Keyed on a *real* transition: revoking an account that holds no
/// key is an idempotent no-op (`204`, nothing appended), so no duplicate
/// `view_grant_revoked` entry is ever minted. The account id comes from the path,
/// so a since-deleted account's key can still be revoked (its handle may be
/// unresolvable — the payload carries the id regardless). Returns `204 No
/// Content`.
pub(super) async fn revoke_view(
    State(state): State<AppState>,
    Path((id, account_id)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;
    let account = AccountId::new(account_id);

    // Best-effort handle for the entry's sentence; a revoked account resolves to
    // None and the payload falls back to the id alone.
    let handle = state
        .accounts
        .find(account)
        .await?
        .map(|a| a.handle.as_str().to_owned());
    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::ViewGrantRevoked,
        user.id,
        json!({ "account_id": *account, "account_handle": handle }),
        Utc::now(),
    );
    transaction(&*state.database, |uow| {
        Box::pin(async move {
            let mut commissions = uow.commissions();
            if commissions.revoke_view(commission, account).await? {
                drop(commissions);
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
    })
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
