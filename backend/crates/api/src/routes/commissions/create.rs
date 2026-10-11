//! `POST /commissions` — any signed-in User creates a commission they own; no
//! Account required, a user-scoped write. The act itself is the
//! changelog's genesis entry.

use application::commission::{create, list};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::{
    commission::{Commission, CommissionTitle},
    maturity::{Maturity, MaturityRating},
};

use super::{from_wire_timestamp, wire_timestamp};
use crate::generated::{CreateCommissionRequest, CreateCommissionResponse};
use crate::{AppState, extract::CallingUser, problem::Problem};

/// Renders a domain maturity posture as the contract's wire message.
impl From<Maturity> for crate::generated::Maturity {
    fn from(maturity: Maturity) -> Self {
        Self {
            rating: maturity.rating.to_string(),
            graphic: maturity.graphic,
        }
    }
}

/// Renders a domain commission as the create response's created resource.
impl From<Commission> for CreateCommissionResponse {
    fn from(commission: Commission) -> Self {
        let maturity = commission.maturity.map(crate::generated::Maturity::from);
        let linked_channel = commission
            .linked_channel
            .as_ref()
            .map(|channel| channel.as_str().to_owned());
        CreateCommissionResponse {
            id: commission.id.to_string(),
            title: commission.title.as_str().to_owned(),
            lifecycle: commission.lifecycle_step.to_string(),
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

/// Creates a commission owned by the signed-in caller (Draft lifecycle), and
/// records its `created` changelog entry atomically with the row.
///
/// `201 Created` carrying the created resource on success; `422
/// invalid_request` for a missing/malformed body or blank title; `422
/// unknown_maturity_rating` for an out-of-vocabulary `maturity.rating`.
pub(super) async fn create_commission(
    State(state): State<AppState>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<CreateCommissionRequest>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let title = CommissionTitle::try_from(body.title)
        .map_err(|e| Problem::invalid_request(e.to_string()))?;
    let maturity = body
        .maturity
        .map(|input| {
            let rating = MaturityRating::try_from(input.rating.as_str()).map_err(|_| {
                Problem::unknown_maturity_rating(format!(
                    "{:?} is not a maturity rating; expected one of: safe, suggestive, nudity, adult.",
                    input.rating,
                ))
            })?;
            Ok::<_, Problem>(Maturity {
                rating,
                graphic: input.graphic,
            })
        })
        .transpose()?;

    let deadline = body
        .deadline
        .map(|at| {
            from_wire_timestamp(at)
                .ok_or_else(|| Problem::invalid_request("deadline is out of range"))
        })
        .transpose()?;

    let command = create::Command {
        maturity,
        deadline,
        actor_id: actor_id.clone(),
        title,
    };

    let created = state
        .app()
        .commissions()
        .create(command, Utc::now())
        .await?;

    // ⚠️ GAP: create::Output carries only the id, so the resource is read
    // back through list rather than assumed.
    let owned = list::Command { user_id: actor_id };
    let listed = state.app().commissions().list(owned).await?;
    let commission = listed
        .commissions
        .into_iter()
        .find(|commission| commission.id == created.id)
        .ok_or_else(|| {
            Problem::internal_error("The commission was created but could not be read back.")
        })?;

    let body = CreateCommissionResponse::from(commission);
    let response = (StatusCode::CREATED, Json(body)).into_response();
    Ok(response)
}
