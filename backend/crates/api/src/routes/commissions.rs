//! The commissions route group: the commission JSON API (ZMVP-65).
//!
//! `POST /commissions` lets any signed-in User create a commission they own — no
//! Account required (a user-scoped write; ZMVP-47, DD 26247170), so it stays off the
//! account-scoped authorization seam in [`super::accounts`]. Like the rest of the
//! JSON API it returns status codes, not redirects: an unrecognized caller gets a
//! `401`. It is part of the cookie surface, so [`crate::app`] mounts the group under
//! the first-party-`Origin` (CSRF) layer.
//!
//! References: ZMVP-65; DESIGN/Commission (`3276807`), Ask-for-Art (`28114957`) D0.

use anyhow::Result;
use axum::{
    Json, Router,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
};
use chrono::Utc;
use domain::{
    datetime::DateTimeUtc,
    elements::{commission::Commission, user::UserId},
    ports::transaction,
};
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, SESSION_USER_KEY, problem::Problem};

/// The commissions route group: creating a commission (`POST /commissions`). On the
/// cookie surface; the composition root wraps the group with the CSRF
/// [`require_first_party_origin`](super::require_first_party_origin) layer.
pub(crate) fn commissions_router() -> Router<AppState> {
    Router::new().route("/commissions", post(create_commission))
}

/// The `POST /commissions` request body: the commission's fixed metadata a caller
/// supplies. The `title` is required (a missing/invalid body is a `422`); `deadline`
/// is the optional envelope field. Owner and lifecycle are not accepted from the
/// client — the owner is the authenticated caller and the lifecycle is always `Draft`.
#[derive(Deserialize)]
struct CreateCommissionBody {
    title: String,
    deadline: Option<DateTimeUtc>,
}

/// Create a commission owned by the signed-in caller (ZMVP-65).
///
/// Resolves the session to the acting [`User`](domain::elements::user::User) —
/// an absent/unreadable session or a vanished User is a `401`, never a redirect,
/// because the frontend *calls* this endpoint. Requires only authentication, no
/// Account (ZMVP-47). Builds the commission with the caller as owner and `Draft`
/// lifecycle, then persists it in one unit of work via
/// [`transaction`](domain::ports::transaction). Returns `201 Created` on success;
/// a missing/invalid JSON body is a `422` (`invalid_request`).
async fn create_commission(
    State(state): State<AppState>,
    session: Session,
    body: Result<Json<CreateCommissionBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let id = session
        .get::<Uuid>(SESSION_USER_KEY)
        .await
        .ok()
        .flatten()
        .ok_or_else(Problem::not_authenticated)?;
    let user = state
        .users
        .find(UserId::new(id))
        .await
        .ok()
        .flatten()
        .ok_or_else(Problem::not_authenticated)?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Title needed."))?;

    let now = Utc::now();
    let commission = Commission::create(body.title, user.id, now, body.deadline);

    transaction(&*state.database, |uow| {
        Box::pin(async move { uow.commissions().create(&commission).await })
    })
    .await?;

    Ok(StatusCode::CREATED.into_response())
}
