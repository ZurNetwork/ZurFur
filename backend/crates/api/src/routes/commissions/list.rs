//! `GET /commissions` — the signed-in user's owned commissions, owner-POV
//! only. Response types are the contract's generated messages.

use application::commission::list;
use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};

use super::wire_timestamp;
use crate::generated::{Commission, ListCommissionsResponse, Maturity};
use crate::{AppState, extract::CallingUser, problem::Problem};

/// Renders a domain commission into the contract's envelope.
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

/// Lists the signed-in user's owned commissions, excluding archived ones.
/// Ordered by id (creation order); no pagination.
///
/// Outcomes:
/// - `200 { "commissions": [ { "id", "title", "lifecycle", "visibility",
///   "deadline"?, "maturity"?, "directionStatus"?, "deadlineStatus"?,
///   "linkedChannel"?, "createdAt" }, … ] }` — absent optionals omit their keys
/// - `401` — not signed in
pub(super) async fn list_commissions(
    State(state): State<AppState>,
    CallingUser(user_id): CallingUser,
) -> Result<Response, Problem> {
    let command = list::Command { user_id };

    let listed = state.app().commissions().list(command).await?;
    let commissions: Vec<Commission> = listed
        .commissions
        .into_iter()
        .map(wire_commission)
        .collect();

    let body = ListCommissionsResponse { commissions };
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}
