//! `POST /commissions` — any signed-in User creates a commission they own
//! (ZMVP-65; no Account required, a user-scoped write — ZMVP-47, DD 26247170),
//! and the act itself is the changelog's genesis entry (ZMVP-87; the Changelog
//! DD's taxonomy includes "creation itself").

use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, Commission, CommissionTitle, NewChangelogEntry},
        maturity::{Maturity, MaturityRating},
    },
    ports::transaction,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;

use crate::{AppState, problem::Problem};

/// The `POST /commissions` request body: the commission's fixed metadata a caller
/// supplies. The `title` is required (a missing/invalid body is a `422`); `deadline`
/// and `maturity` are the optional envelope fields. Owner and lifecycle are not
/// accepted from the client — the owner is the authenticated caller and the
/// lifecycle is always `Draft`.
///
/// `maturity` lets the caller rate the commission **at creation** instead of a
/// follow-up `PUT /commissions/{id}/maturity`: when present its `rating` is resolved
/// server-side (a bad token is a `422`, never stored) and lands in the same write;
/// when absent the commission is born unrated (`None`) — legal while it stays
/// private (Maturity Vocabulary DD `29982722`: a rating is required at *publish*,
/// not at birth).
#[derive(Deserialize)]
pub(super) struct CreateCommissionBody {
    title: String,
    deadline: Option<DateTimeUtc>,
    maturity: Option<MaturityInput>,
}

/// The optional at-creation maturity posture on [`CreateCommissionBody`] — the same
/// `{ rating, graphic }` shape the `PUT .../maturity` route accepts, so rating at
/// birth and re-rating later speak one wire language. `graphic` defaults to `false`
/// when omitted.
#[derive(Deserialize)]
struct MaturityInput {
    rating: String,
    #[serde(default)]
    graphic: bool,
}

/// Create a commission owned by the signed-in caller (ZMVP-65), recording the
/// creation in its changelog (ZMVP-87).
///
/// Resolves the session to the acting [`User`](domain::elements::user::User) via
/// [`current_user`](super::current_user) — an absent session or vanished User is
/// a `401`, never a redirect, because the frontend *calls* this endpoint.
/// Requires only authentication, no Account (ZMVP-47). Builds the commission
/// with the caller as owner and `Draft` lifecycle, then persists it **and its
/// `created` changelog entry in one unit of work** — the entry commits
/// atomically with the row it records (Changelog DD D4), so a commission can
/// never exist without its genesis entry from this ticket on (commissions
/// created before ZMVP-87 landed are deliberately not backfilled). The root
/// surface of the content tree is minted **inside** the store write itself
/// ([`CommissionWrites::create`](domain::ports::CommissionWrites::create),
/// ZMVP-71), not here — no handler can create a treeless commission. Returns
/// `201 Created` on success. A missing/malformed JSON body — or a blank
/// (empty/whitespace) title, rejected by
/// [`CommissionTitle`](domain::elements::commission::CommissionTitle)'s
/// `TryFrom<String>` —
/// is a `422` (`invalid_request`). An optional `maturity` posture may rate the
/// commission at birth; its `rating` is validated server-side, and an
/// out-of-vocabulary token is a `422` (`unknown_maturity_rating`) before any write.
pub(super) async fn create_commission(
    State(state): State<AppState>,
    session: Session,
    body: Result<Json<CreateCommissionBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let title = CommissionTitle::try_from(body.title)
        .map_err(|e| Problem::invalid_request(e.to_string()))?;
    // The optional at-creation rating passes the same server-side enum gate as the
    // PUT route — an out-of-vocabulary token is a 422 here, before anything is
    // written, never a silently-dropped or defaulted value.
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

    let now = Utc::now();
    let mut commission = Commission::create(title, user.id, now, body.deadline);
    commission.maturity = maturity;
    // The genesis entry: the payload carries the title so the sentence renders
    // without joins (the DD's core-renderable rule).
    let entry = NewChangelogEntry::event(
        commission.id,
        ChangelogEntryKind::Created,
        user.id,
        json!({ "title": commission.title.as_str() }),
        now,
    );

    transaction(&*state.database, |uow| {
        Box::pin(async move {
            uow.commissions().create(&commission).await?;
            uow.changelog().append(&entry).await
        })
    })
    .await?;

    Ok(StatusCode::CREATED.into_response())
}
