//! `PUT /commissions/{id}/maturity` — the owner rates the commission (Safe /
//! Suggestive / Nudity / Adult plus a Graphic flag).
//! Replace-only: no `DELETE` sibling, so a rating can never clear.

use application::commission::maturity::set;
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::maturity::MaturityRating;
use serde::Deserialize;

use crate::{
    AppState,
    extract::{CallingUser, CommissionPath},
    problem::{FieldPointer, Problem},
};

/// The `PUT /commissions/{id}/maturity` request body: the rating token, plus
/// the optional Graphic flag (defaults to `false`).
#[derive(Deserialize)]
pub(super) struct SetMaturityBody {
    rating: String,
    #[serde(default)]
    graphic: bool,
}

/// Rates (or re-rates) the commission. Owner-only; `422
/// unknown_maturity_rating` at `#/rating` for a token outside the vocabulary,
/// before anyone's standing is checked; idempotent on an unchanged rating.
/// Returns `204 No Content`.
pub(super) async fn set_maturity(
    State(state): State<AppState>,
    CallingUser(actor_id): CallingUser,
    CommissionPath(commission_id): CommissionPath,
    body: Result<Json<SetMaturityBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let rating = body.rating.parse::<MaturityRating>().map_err(|_| {
        let pointer = FieldPointer::member("rating");
        Problem::unknown_maturity_rating_at(pointer)
    })?;
    let command = set::Command {
        actor_id,
        commission_id,
        graphic: body.graphic,
        maturity_rating: rating,
    };

    state
        .app()
        .commissions()
        .maturity()
        .run(command, Utc::now())
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
