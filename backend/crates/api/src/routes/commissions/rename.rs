//! `PATCH /commissions/{id}/title` — the owner renames a commission.

use application::commission::{CommissionValues, rename};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::commission::{CommissionTitle, CommissionTitleError};

use super::{title_refused, title_write_problem, wire_timestamp};
use crate::generated::{RenameCommissionRequest, RenameCommissionResponse};
use crate::{
    AppState,
    extract::{CallingUser, CommissionPath},
    problem::{FieldCode, Problem},
};

/// Renders the renamed commission, as stored, as the rename response's resource.
impl From<CommissionValues> for RenameCommissionResponse {
    fn from(commission: CommissionValues) -> Self {
        let maturity = commission.maturity.map(crate::generated::Maturity::from);
        let linked_channel = commission
            .linked_channel
            .as_ref()
            .map(|channel| channel.as_str().to_owned());
        RenameCommissionResponse {
            id: commission.id.to_string(),
            title: commission.title.as_str().to_owned(),
            lifecycle: commission.lifecycle.to_string(),
            visibility: commission.visibility.to_string(),
            deadline: commission.deadline.map(wire_timestamp),
            maturity,
            direction_status: commission.direction_status.map(|status| status.to_string()),
            deadline_status: commission.deadline_status.map(|status| status.to_string()),
            linked_channel,
            created_at: Some(wire_timestamp(commission.created_at)),
        }
    }
}

/// Renames the commission the path names; an `id` in the body is ignored.
///
/// `200` with the commission as stored; `422 invalid_request` for a malformed body, or with
/// a field error at `#/title` for a title that is blank or the name rule refuses; `403` for
/// a Participant who isn't the owner; `404` for anyone else and for an id that names nothing.
pub(super) async fn rename_commission(
    State(state): State<AppState>,
    CallingUser(actor_id): CallingUser,
    CommissionPath(commission_id): CommissionPath,
    body: Result<Json<RenameCommissionRequest>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let title = CommissionTitle::try_from(body.title)
        .map_err(|CommissionTitleError::Empty| title_refused(FieldCode::NameEmpty))?;

    let command = rename::Command {
        actor_id,
        commission_id,
        title,
    };
    let renamed = state
        .app()
        .commissions()
        .rename(command, Utc::now())
        .await
        .map_err(title_write_problem)?;

    let body = RenameCommissionResponse::from(renamed);
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}
