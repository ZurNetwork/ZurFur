//! `POST /commissions` — any signed-in User creates a commission they own
//! (ZMVP-65; no Account required, a user-scoped write — ZMVP-47, DD 26247170),
//! and the act itself is the changelog's genesis entry (ZMVP-87; the Changelog
//! DD's taxonomy includes "creation itself").

use application::commission::{create, list};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::{
    commission::CommissionTitle,
    maturity::{Maturity, MaturityRating},
};

use super::{from_wire_timestamp, list::wire_commission};
use crate::generated::{Commission, CreateCommissionRequest, CreateCommissionResponse};
use crate::{AppState, extract::CallingUser, problem::Problem};

/// The created resource, in the create response's envelope. Both messages
/// carry the same ten fields, so the create body is the listing row moved
/// across — one projection ([`wire_commission`]), never two that can drift
/// (`golden_wire` asserts the two render identically).
impl From<Commission> for CreateCommissionResponse {
    fn from(commission: Commission) -> Self {
        let Commission {
            id,
            title,
            lifecycle,
            visibility,
            deadline,
            maturity,
            direction_status,
            deadline_status,
            linked_channel,
            created_at,
        } = commission;
        CreateCommissionResponse {
            id,
            title,
            lifecycle,
            visibility,
            deadline,
            maturity,
            direction_status,
            deadline_status,
            linked_channel,
            created_at,
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

    // ⚠️ GAP (ZMVP-205): the contract's `CreateCommissionResponse` is the whole
    // resource — "create returns the created resource" (ruling 2026-07-25,
    // pinned by `golden_wire`) — but `create::Output` carries only the id, and
    // the birth lifecycle/visibility are the domain's to state, not this
    // driver's to assume. Until `create::Output` carries the commission's
    // values, the resource is read back through the one use case that projects
    // commissions, so the response can never disagree with the stored row.
    let owned = list::Command { user_id: actor_id };
    let listed = state.app().commissions().list(owned).await?;
    let commission = listed
        .commissions
        .into_iter()
        .find(|commission| commission.id == created.id)
        .ok_or_else(|| {
            Problem::internal_error("The commission was created but could not be read back.")
        })?;

    let body = CreateCommissionResponse::from(wire_commission(commission));
    let response = (StatusCode::CREATED, Json(body)).into_response();
    Ok(response)
}
