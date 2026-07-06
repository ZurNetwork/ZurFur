//! `PUT /commissions/{id}/maturity` — the owner rates the commission
//! (ZMVP-31; Maturity Vocabulary DD `29982722`): one of Safe / Suggestive /
//! Nudity / Adult plus the orthogonal Graphic flag, validated **server-side**
//! against the domain enum — never merely hidden client-side.
//!
//! A commission is born unrated (`maturity` null) and may stay so while
//! Private; the widening gate (ZMVP-74) is what makes the rating *required*
//! before anything shows to non-participants — this route owns the field, not
//! the gate. **Replace-only**: there is deliberately no `DELETE` sibling, so a
//! rating can change but never clear — a widened commission can't quietly
//! return to unrated. Deliberately not changelog-recorded (maturity edits are
//! not in the frozen ZMVP-87 taxonomy).

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{
    elements::{
        commission::CommissionId,
        maturity::{Maturity, MaturityRating},
    },
    ports::transaction,
};
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// The `PUT /commissions/{id}/maturity` request body: the rating token, plus
/// the optional Graphic flag (omitted = not graphic — the flag "rides
/// alongside" a rating, DD Decision 2, so it defaults off rather than being
/// required ceremony).
#[derive(Deserialize)]
pub(super) struct SetMaturityBody {
    rating: String,
    #[serde(default)]
    graphic: bool,
}

/// Rate (or re-rate) the commission (ZMVP-31).
///
/// Owner-only ([`require_owner`] — the shared managing-authority gate, so a
/// non-participant gets the uniform closed-door 404). The rating is resolved
/// through [`MaturityRating::parse`] — the **server-side** enum gate: a token
/// outside the vocabulary is a `422`
/// ([`Problem::unknown_maturity_rating`]), never stored, never defaulted. The
/// posture lands as one envelope write on a unit of work. Returns
/// `204 No Content`.
pub(super) async fn set_maturity(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<SetMaturityBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let rating = MaturityRating::try_from(body.rating.as_str()).map_err(|_| {
        Problem::unknown_maturity_rating(format!(
            "{:?} is not a maturity rating; expected one of: safe, suggestive, nudity, adult.",
            body.rating,
        ))
    })?;
    let maturity = Maturity {
        rating,
        graphic: body.graphic,
    };

    transaction(&*state.database, |uow| {
        Box::pin(async move { uow.commissions().set_maturity(commission, maturity).await })
    })
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
