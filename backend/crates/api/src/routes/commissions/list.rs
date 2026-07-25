//! `GET /commissions` — the signed-in user's OWNED commissions, owner-POV only
//! (ZMVP-157; frontend enablement for ZMVP-153 AC1). The non-participant
//! projection view (ZMVP-75) is a distinct, later surface this does not
//! attempt: nothing here answers "what can I see as a seated participant",
//! only "what do I own".

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::datetime::DateTimeUtc;
use domain::elements::commission::Commission;
use serde::Serialize;
use tower_sessions::Session;

use crate::{AppState, problem::Problem};

/// A commission's maturity rating as the API renders it (DD `29982722`): the
/// atproto self-label axis plus the orthogonal `graphic` flag.
#[derive(Serialize)]
struct MaturityBody {
    /// The maturity axis value — `safe` / `suggestive` / `nudity` / `adult`.
    rating: String,
    /// The orthogonal graphic-content flag.
    graphic: bool,
}

/// One row of `GET /commissions` — the envelope fields a listing renders.
///
/// The content tree is deliberately absent (that stays the future
/// single-commission surface's job), and `owner` is omitted because this
/// endpoint is owner-POV only: every row's owner is always the caller.
///
/// Named rather than built with `json!` (ZMVP-158): a schema cannot be checked
/// against a `json!` literal because there is no type to check. This is also
/// the **first place a `Commission` is serialized anywhere in the API**, so
/// this field set is the precedent every later commission surface inherits.
///
/// Optional fields are `Option<T>` and currently serialize as explicit `null`
/// — the shape `GET /commissions` has always emitted. ProtoJSON would omit the
/// key instead; that difference is recorded as an open wire-format question
/// (`.understand/20260725-zmvp-159-wire-format-fork.md`) rather than changed
/// here, because this ticket is a pure refactor.
#[derive(Serialize)]
struct CommissionBody {
    /// The commission's UUIDv7, rendered as a string.
    id: String,
    /// The commission's title.
    title: String,
    /// The lifecycle step — `draft` / `batched` / `active` / …
    lifecycle: String,
    /// Who may see it.
    visibility: String,
    /// The agreed deadline, if one is set.
    deadline: Option<DateTimeUtc>,
    /// The maturity rating, if one is set.
    maturity: Option<MaturityBody>,
    /// The direction axis of the two-dimensional status set, if set.
    direction_status: Option<String>,
    /// The deadline axis of the two-dimensional status set, if set.
    deadline_status: Option<String>,
    /// The external chat channel pointer, if one is linked.
    linked_channel: Option<String>,
    /// When the commission was created.
    created_at: DateTimeUtc,
}

impl From<Commission> for CommissionBody {
    fn from(commission: Commission) -> Self {
        let maturity = commission.maturity.map(|maturity| MaturityBody {
            rating: maturity.rating.as_str().to_owned(),
            graphic: maturity.graphic,
        });
        Self {
            id: commission.id.to_string(),
            title: commission.title.as_str().to_owned(),
            lifecycle: commission.lifecycle_step.as_str().to_owned(),
            visibility: commission.visibility.as_str().to_owned(),
            deadline: commission.deadline,
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
            created_at: commission.created_at,
        }
    }
}

/// List the signed-in user's owned commissions (ZMVP-157).
///
/// Resolves the session to the acting [`User`](domain::elements::user::User)
/// via [`current_user`](super::current_user) — an absent session or vanished
/// User is a `401`, never a redirect, because the frontend *calls* this
/// endpoint (consistent with `GET /me`). **Archived commissions are
/// excluded** — an archived commission is meant to disappear from active
/// views (Deletion DD `3014657`; ZMVP-68), and this listing is exactly that
/// active-view filter (`Commission::archived_at`'s documented
/// listing-projection contract; [`CommissionStore::list_owned_by`](domain::ports::CommissionStore::list_owned_by)).
/// Ordered by id (UUIDv7 sorts as creation order); no pagination (v1 volumes
/// are small).
///
/// Outcomes:
/// - `200 [ { "id", "title", "lifecycle", "visibility", "deadline", "maturity",
///   "direction_status", "deadline_status", "linked_channel", "created_at" }, … ]`
/// - `401` — not signed in
pub(super) async fn list_commissions(
    State(state): State<AppState>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;

    let commissions = state.commissions.list_owned_by(user.id).await?;
    let rows: Vec<CommissionBody> = commissions.into_iter().map(CommissionBody::from).collect();

    let response = (StatusCode::OK, Json(rows)).into_response();
    Ok(response)
}
