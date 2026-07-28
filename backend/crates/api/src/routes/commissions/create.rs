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
    elements::{
        commission::{ChangelogEntryKind, Commission, CommissionTitle, NewChangelogEntry},
        maturity::{Maturity, MaturityRating},
    },
    ports::UnitOfWork,
};
use serde_json::json;
use tower_sessions::Session;

use super::{from_wire_timestamp, list::wire_commission};
use crate::generated::{CreateCommissionRequest, CreateCommissionResponse};
use crate::{AppState, problem::Problem};

/// The request shape is the contract's GENERATED `CreateCommissionRequest`
/// (ZMVP-160): `title` required; `deadline` and `maturity` optional. Owner and
/// lifecycle are never accepted from the client — the owner is the caller, the
/// lifecycle is always Draft. The generated deserializer REJECTS unknown
/// request fields (canonical ProtoJSON; the contract's tolerant-reader duty is
/// response-side and client-only — the server stays conservative, VERSIONING
/// §6), and parses `deadline` under the STRICT Timestamp grammar, closing the
/// chrono-laxness input gap (§7.3).
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
    body: Result<Json<CreateCommissionRequest>, JsonRejection>,
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

    let deadline = body
        .deadline
        .map(|at| {
            from_wire_timestamp(at)
                .ok_or_else(|| Problem::invalid_request("deadline is out of range"))
        })
        .transpose()?;

    let now = Utc::now();
    let mut commission = Commission::create(title, user.id, now, deadline);
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

    // The closure owns what it writes and hands the committed commission back
    // out — the create_account pattern — because the response now CARRIES it
    // (contract, Engineer ruling 2026-07-25): the interface renders what the
    // program tells it, and create-then-navigate needs the id. Minted at
    // /api/v1; the pre-GA surface answered an empty 201.
    let commission = state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().create(&commission).await?;
            uow.changelog().append(&entry).await?;
            Ok(commission)
        })
        .await?;

    let row = wire_commission(commission);
    let body = CreateCommissionResponse {
        id: row.id,
        title: row.title,
        lifecycle: row.lifecycle,
        visibility: row.visibility,
        deadline: row.deadline,
        maturity: row.maturity,
        direction_status: row.direction_status,
        deadline_status: row.deadline_status,
        linked_channel: row.linked_channel,
        created_at: row.created_at,
    };
    let response = (StatusCode::CREATED, Json(body)).into_response();
    Ok(response)
}
