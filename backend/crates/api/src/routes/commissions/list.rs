//! `GET /commissions` — the signed-in user's OWNED commissions, owner-POV only
//! (ZMVP-157; frontend enablement for ZMVP-153 AC1). The non-participant
//! projection view (ZMVP-75) is a distinct, later surface this does not
//! attempt: nothing here answers "what can I see as a seated participant",
//! only "what do I own".

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::elements::commission::Commission;
use serde_json::json;
use tower_sessions::Session;

use crate::{AppState, problem::Problem};

/// Serialize a [`Commission`] for the `GET /commissions` list — the envelope
/// fields a listing renders. The content tree is deliberately not loaded here
/// (that stays the future single-commission surface's job); `owner` is
/// omitted since this endpoint is owner-POV only — every row's owner is
/// always the caller.
fn commission_json(commission: Commission) -> serde_json::Value {
    let maturity = commission.maturity.map(|maturity| {
        json!({
            "rating": maturity.rating.as_str(),
            "graphic": maturity.graphic,
        })
    });
    json!({
        "id": commission.id.to_string(),
        "title": commission.title.as_str(),
        "lifecycle": commission.lifecycle_step.as_str(),
        "visibility": commission.visibility.as_str(),
        "deadline": commission.deadline,
        "maturity": maturity,
        "direction_status": commission.direction_status.map(|status| status.as_str()),
        "deadline_status": commission.deadline_status.map(|status| status.as_str()),
        "linked_channel": commission.linked_channel.as_ref().map(|c| c.as_str()),
        "created_at": commission.created_at,
    })
}

/// List the signed-in user's owned commissions (ZMVP-157).
///
/// Resolves the session to the acting [`User`](domain::elements::user::User)
/// via [`current_user`](super::current_user) — an absent session or vanished
/// User is a `401`, never a redirect, because the frontend *calls* this
/// endpoint (consistent with `GET /me`). **Archived commissions are
/// excluded** — an archived commission is meant to disappear from active
/// views (Deletion DD `3014657`; ZMVP-68), and this listing is exactly that
/// active-view filter (`Commission::archived_at`'s documented
/// listing-projection contract; [`CommissionStore::list_owned_by`](domain::ports::CommissionStore::list_owned_by)).
/// Ordered by id (UUIDv7 sorts as creation order); no pagination (v1 volumes
/// are small).
///
/// Outcomes:
/// - `200 [ { "id", "title", "lifecycle", "visibility", "deadline", "maturity",
///   "direction_status", "deadline_status", "linked_channel", "created_at" }, … ]`
/// - `401` — not signed in
pub(super) async fn list_commissions(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;

    let commissions = state.commissions.list_owned_by(user.id).await?;
    let body: Vec<serde_json::Value> = commissions.into_iter().map(commission_json).collect();

    Ok((StatusCode::OK, axum::Json(serde_json::Value::Array(body))).into_response())
}
