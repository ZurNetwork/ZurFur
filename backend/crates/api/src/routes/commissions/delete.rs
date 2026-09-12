//! `DELETE /commissions/{id}` — the owner hard-deletes a fact-free commission.
//! A fact-bearing commission is refused toward
//! Archive instead.

use application::commission::delete;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::elements::commission::CommissionId;

use crate::{AppState, extract::CallingUser, problem::Problem};

/// Hard-deletes a fact-free commission. Owner-only; `404` for a caller who
/// may not see it. Fact-free → `204 No Content`; fact-bearing → `409
/// commission_has_facts` pointing the caller at Archive.
pub(super) async fn delete_commission(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    let command = delete::Command {
        actor_id,
        commission_id,
    };

    let delete::Output { outcome } = state.app().commissions().delete(command).await?;

    match outcome {
        delete::Outcome::Deleted => Ok(StatusCode::NO_CONTENT.into_response()),
        delete::Outcome::HasFacts => Err(Problem::commission_has_facts()),
    }
}
