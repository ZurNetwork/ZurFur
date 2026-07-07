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
    elements::commission::{ChangelogEntryKind, Commission, CommissionTitle, NewChangelogEntry},
    ports::transaction,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;

use crate::{AppState, problem::Problem};

/// The `POST /commissions` request body: the commission's fixed metadata a caller
/// supplies. The `title` is required (a missing/invalid body is a `422`); `deadline`
/// is the optional envelope field. Owner and lifecycle are not accepted from the
/// client — the owner is the authenticated caller and the lifecycle is always `Draft`.
#[derive(Deserialize)]
pub(super) struct CreateCommissionBody {
    title: String,
    deadline: Option<DateTimeUtc>,
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
/// created before ZMVP-87 landed are deliberately not backfilled). Returns
/// `201 Created` on success. A missing/malformed JSON body — or a blank
/// (empty/whitespace) title, rejected by
/// [`CommissionTitle::try_new`](domain::elements::commission::CommissionTitle::try_new) —
/// is a `422` (`invalid_request`).
pub(super) async fn create_commission(
    State(state): State<AppState>,
    session: Session,
    body: Result<Json<CreateCommissionBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let title = CommissionTitle::try_new(body.title)
        .map_err(|e| Problem::invalid_request(e.to_string()))?;

    let now = Utc::now();
    let commission = Commission::create(title, user.id, now, body.deadline);
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
