//! `PUT`/`DELETE /commissions/{id}/channel` — declare or clear the commission's
//! external **linked channel** pointer (ZMVP-87 AC3; Changelog DD Decision 2:
//! "a commission may declare where we talk"). Zurfur hosts no chat: the value is
//! raw pointer text (URL or handle) that renders as an opaque pointer and never
//! auto-embeds — so there is **no scheme allowlist**; safe rendering is the
//! frontend's job. Each set/clear is changelog-recorded, atomically.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{ChangelogEntryKind, ChannelPointer, CommissionId, NewChangelogEntry},
    ports::UnitOfWork,
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// The `PUT /commissions/{id}/channel` request body: the raw pointer text.
#[derive(Deserialize)]
pub(super) struct LinkChannelBody {
    channel: String,
}

/// Declare (or replace) the commission's linked channel (ZMVP-87 AC3).
///
/// Owner-only ([`require_owner`]). The pointer is validated by
/// `ChannelPointer`'s `TryFrom<String>` — trimmed, non-empty, length-capped,
/// control-character-free, **no scheme allowlist** — a failure is a `422`. The
/// column write and the `channel_linked` changelog entry (payload carries the
/// pointer, so it renders without joins) land in **one unit of work** (Changelog
/// DD D4), with the append keyed on the write's *changed* answer — re-declaring
/// the identical pointer is an idempotent no-op (`204`, no entry), and the
/// keying holds under concurrent writers because the port decides **inside**
/// the transaction. Returns `204 No Content`.
pub(super) async fn link_channel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<LinkChannelBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let pointer = ChannelPointer::try_from(body.channel)
        .map_err(|e| Problem::invalid_request(e.to_string()))?;

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::ChannelLinked,
        user.id,
        json!({ "channel": pointer.as_str() }),
        Utc::now(),
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            let changed = uow
                .commissions()
                .set_linked_channel(commission, Some(&pointer))
                .await?;
            if changed {
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Clear the commission's linked channel (ZMVP-87 AC3).
///
/// Owner-only ([`require_owner`]). Clearing an already-clear channel is an
/// idempotent no-op — `204` with **no** entry appended (a record of nothing
/// changing would be noise, not audit). Otherwise the column clears and the
/// `channel_unlinked` entry (payload names the pointer that was cleared) lands
/// in one unit of work, keyed on the write's *changed* answer — so two racing
/// clears append exactly one entry (the pre-read below is only a fast path; the
/// port decides **inside** the transaction). Returns `204 No Content`.
pub(super) async fn clear_channel(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    let found = require_owner(&state, commission, &user).await?;

    let Some(previous) = found.linked_channel else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };

    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::ChannelUnlinked,
        user.id,
        json!({ "channel": previous.as_str() }),
        Utc::now(),
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            let changed = uow
                .commissions()
                .set_linked_channel(commission, None)
                .await?;
            if changed {
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
