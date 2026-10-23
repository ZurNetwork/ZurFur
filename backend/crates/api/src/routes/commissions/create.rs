//! `POST /commissions` — any signed-in User creates a commission they own; no
//! Account required, a user-scoped write. The act itself is the
//! changelog's genesis entry.

use application::commission::{CommissionValues, create};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::{
    commission::{CommissionTitle, CommissionTitleError},
    maturity::{Maturity, MaturityRating},
};

use super::{from_wire_timestamp, title_refused, title_write_problem, wire_timestamp};
use crate::generated::{CreateCommissionRequest, CreateCommissionResponse};
use crate::{
    AppState,
    extract::CallingUser,
    problem::{FieldCode, Problem},
};

/// Renders a domain maturity posture as the contract's wire message.
impl From<Maturity> for crate::generated::Maturity {
    fn from(maturity: Maturity) -> Self {
        Self {
            rating: maturity.rating.to_string(),
            graphic: maturity.graphic,
        }
    }
}

/// Renders the created commission, as stored, as the create response's resource.
impl From<CommissionValues> for CreateCommissionResponse {
    fn from(commission: CommissionValues) -> Self {
        let maturity = commission.maturity.map(crate::generated::Maturity::from);
        let linked_channel = commission
            .linked_channel
            .as_ref()
            .map(|channel| channel.as_str().to_owned());
        CreateCommissionResponse {
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

/// Creates a commission owned by the signed-in caller (Draft lifecycle), and
/// records its `created` changelog entry atomically with the row.
///
/// `201 Created` carrying the created resource on success; `422
/// invalid_request` for a malformed body; `422 invalid_request` with a field
/// error at `#/title` for a blank or missing title (a missing title decodes as
/// empty, so it takes the same path) or one the name rule refuses; `422
/// unknown_maturity_rating` for an out-of-vocabulary `maturity.rating`.
pub(super) async fn create_commission(
    State(state): State<AppState>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<CreateCommissionRequest>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let title = CommissionTitle::try_from(body.title)
        .map_err(|CommissionTitleError::Empty| title_refused(FieldCode::NameEmpty))?;
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
        actor_id,
        title,
    };

    let created = state
        .app()
        .commissions()
        .create(command, Utc::now())
        .await
        .map_err(title_write_problem)?;

    let body = CreateCommissionResponse::from(created);
    let response = (StatusCode::CREATED, Json(body)).into_response();
    Ok(response)
}
