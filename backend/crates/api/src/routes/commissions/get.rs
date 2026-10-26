//! `GET /commissions/{id}` — read one commission as anyone the Den opens it to sees it.

use application::commission::get;
use axum::{
    Json,
    extract::State,
    response::{IntoResponse, Response},
};
use chrono::Utc;

use super::wire_timestamp;
use crate::generated::GetCommissionResponse;
use crate::{
    AppState,
    extract::{CallingUser, CommissionPath},
    problem::Problem,
};

/// Renders the commission as the read's resource: the fields every opener may see, the acts by
/// their words and the lifecycle targets by their step tokens.
impl From<get::Output> for GetCommissionResponse {
    fn from(commission: get::Output) -> Self {
        let maturity = commission.maturity.map(crate::generated::Maturity::from);
        let acts = commission.acts.iter().map(ToString::to_string).collect();
        let lifecycle_targets = commission
            .lifecycle_targets
            .iter()
            .map(ToString::to_string)
            .collect();
        GetCommissionResponse {
            id: commission.id.to_string(),
            title: commission.title.to_string(),
            lifecycle: commission.lifecycle.to_string(),
            visibility: commission.visibility.to_string(),
            deadline: commission.deadline.map(wire_timestamp),
            maturity,
            direction_status: commission.direction_status.map(|status| status.to_string()),
            deadline_status: commission.deadline_status.map(|status| status.to_string()),
            created_at: Some(wire_timestamp(commission.created_at)),
            archived: commission.archived,
            acts,
            lifecycle_targets,
        }
    }
}

/// Reads the commission the path names.
///
/// `200` with the commission, its deadline status at the request's time and the acts the viewer
/// may take; `404` for anyone the Den doesn't open it to and for an id that names nothing.
pub(super) async fn get_commission(
    State(state): State<AppState>,
    CallingUser(actor_id): CallingUser,
    CommissionPath(commission_id): CommissionPath,
) -> Result<Response, Problem> {
    let query = get::Query {
        actor_id,
        commission_id,
    };
    let commission = state
        .app()
        .commissions()
        .get(query, Utc::now())
        .await
        .map_err(Problem::from)?;

    let body = GetCommissionResponse::from(commission);
    Ok(Json(body).into_response())
}
