//! `GET /commissions` — the signed-in user's OWNED commissions, owner-POV only
//! (ZMVP-157; frontend enablement for ZMVP-153 AC1). The non-participant
//! projection view (ZMVP-75) is a distinct, later surface this does not
//! attempt: nothing here answers "what can I see as a seated participant",
//! only "what do I own".
//!
//! The response types are the contract's GENERATED messages (ZMVP-160):
//! `Commission` / `Maturity` / `ListCommissionsResponse` from
//! `contract/zurfur/api/v1/commission.proto` — a shape that drifts from the
//! corpus stops compiling, which is the property the contract exists for.
//! Their serde is canonical ProtoJSON: lowerCamelCase keys (R1), absent
//! optionals omit their keys (R4).

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use tower_sessions::Session;

use super::wire_timestamp;
use crate::generated::{Commission, ListCommissionsResponse, Maturity};
use crate::{AppState, problem::Problem};

/// Render a domain commission into the contract's envelope. The ONE mapping
/// site for the listing row; `create` builds its own response message from the
/// same fields (the corpus keeps the two messages separate on purpose, so each
/// endpoint's response can evolve independently).
pub(super) fn wire_commission(commission: domain::elements::commission::Commission) -> Commission {
    let maturity = commission.maturity.map(|maturity| Maturity {
        rating: maturity.rating.as_str().to_owned(),
        graphic: maturity.graphic,
    });
    Commission {
        id: commission.id.to_string(),
        title: commission.title.as_str().to_owned(),
        lifecycle: commission.lifecycle_step.as_str().to_owned(),
        visibility: commission.visibility.as_str().to_owned(),
        deadline: commission.deadline.map(wire_timestamp),
        maturity,
        direction_status: commission
            .direction_status
            .map(|status| status.as_str().to_owned()),
        deadline_status: commission
            .deadline_status
            .map(|status| status.as_str().to_owned()),
        linked_channel: commission
            .linked_channel
            .as_ref()
            .map(|channel| channel.as_str().to_owned()),
        created_at: Some(wire_timestamp(commission.created_at)),
    }
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
/// - `200 { "commissions": [ { "id", "title", "lifecycle", "visibility",
///   "deadline"?, "maturity"?, "directionStatus"?, "deadlineStatus"?,
///   "linkedChannel"?, "createdAt" }, … ] }` — absent optionals omit their keys
/// - `401` — not signed in
pub(super) async fn list_commissions(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;

    let commissions = state.commissions.list_owned_by(user.id).await?;
    let commissions: Vec<Commission> = commissions.into_iter().map(wire_commission).collect();

    let body = ListCommissionsResponse { commissions };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}
