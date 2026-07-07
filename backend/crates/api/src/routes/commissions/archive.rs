//! `POST /commissions/{id}/archive` / `POST /commissions/{id}/unarchive` — the
//! owner archives, or un-archives, a commission (ZMVP-68). Two named acts
//! rather than a PUT/DELETE flag resource (Engineer ruling 2026-07-06 on PR
//! #104): the routes say what the changelog entries say.
//!
//! Archive is the **soft** path of the Deletion DD (`3014657`) — the mandatory
//! one once facts exist, and available regardless of facts (hard delete,
//! ZMVP-66, is the fact-gated path): the record and its facts survive intact
//! and stay queryable by Participants; only the active-view listings lose the
//! commission. Un-archive exists as an explicit owner act returning it to
//! active views, and **both directions are changelog entries** (Engineer ruling
//! 2026-07-05, recorded on the ticket). Owner-only in both directions — archive
//! sits in the owner-only reserve even once Commission Admin lands (Structural
//! Authority DD `29425666` Decision 2).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{ChangelogEntryKind, CommissionId, NewChangelogEntry},
    ports::transaction,
};
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// Archive the commission (ZMVP-68 AC1) — it leaves the active views; the
/// record survives.
///
/// Owner-only ([`require_owner`](super::require_owner)); a non-participant gets
/// the uniform 404 (the closed door). The flag write and the `archived`
/// changelog entry (payload carries the title, so the sentence renders without
/// joins) land in **one unit of work**, and the entry is keyed on the store
/// reporting a *real* transition — so archiving an already-archived commission
/// is an idempotent no-op (`204`, nothing appended, original stamp kept; the
/// clear-channel precedent: a record of nothing changing would be noise, not
/// audit). Returns `204 No Content`.
pub(super) async fn archive_commission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    let found = require_owner(&state, commission, &user).await?;

    let now = Utc::now();
    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::Archived,
        user.id,
        json!({ "title": found.title.as_str() }),
        now,
    );
    transaction(&*state.database, |uow| {
        Box::pin(async move {
            let mut commissions = uow.commissions();
            if commissions.set_archived(commission, Some(now)).await? {
                drop(commissions);
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
    })
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Un-archive the commission — the explicit owner act returning it to active
/// views (ZMVP-68; Engineer ruling 2026-07-05).
///
/// Owner-only ([`require_owner`](super::require_owner)), same closed-door 404
/// for outsiders. Mirrors [`archive_commission`]: the flag clear and the
/// `unarchived` entry land in one unit of work, keyed on a real transition —
/// un-archiving a commission that is not archived is an idempotent no-op
/// (`204`, nothing appended). Returns `204 No Content`.
pub(super) async fn unarchive_commission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    let found = require_owner(&state, commission, &user).await?;

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::Unarchived,
        user.id,
        json!({ "title": found.title.as_str() }),
        Utc::now(),
    );
    transaction(&*state.database, |uow| {
        Box::pin(async move {
            let mut commissions = uow.commissions();
            if commissions.set_archived(commission, None).await? {
                drop(commissions);
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
    })
    .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
