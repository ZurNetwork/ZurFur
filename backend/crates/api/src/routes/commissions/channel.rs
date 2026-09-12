//! `PUT`/`DELETE /commissions/{id}/channel` — declare or clear the commission's
//! external linked-channel pointer.

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

use super::require_owner;
use crate::{AppState, extract::CallingUser, problem::Problem};

/// The `PUT /commissions/{id}/channel` request body: the raw pointer text.
#[derive(Deserialize)]
pub(super) struct LinkChannelBody {
    channel: String,
}

/// Declares (or replaces) the commission's linked channel. Owner-only;
/// `422` on an invalid pointer. Idempotent — re-declaring the same pointer is
/// a no-op. Returns `204 No Content`.
#[deprecated(note = "Moving completely to a plugin")]
pub(super) async fn link_channel(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<LinkChannelBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // TODO(engineer): no use case exists yet, so this still authorizes and
    // transacts in the driver (DD 55836674 D6/D7 place both in the application layer).
    require_owner(&state, &commission_id, &actor_id).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let pointer = ChannelPointer::try_from(body.channel)
        .map_err(|e| Problem::invalid_request(e.to_string()))?;

    let entry = NewChangelogEntry::event(
        commission_id,
        ChangelogEntryKind::ChannelLinked,
        actor_id,
        json!({ "channel": pointer.as_str() }),
        Utc::now(),
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            let changed = uow
                .commissions()
                .set_linked_channel(&commission_id, Some(&pointer))
                .await?;
            if changed {
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Clears the commission's linked channel. Owner-only and idempotent — no
/// entry is appended if there was nothing to clear. Returns `204 No Content`.
pub(super) async fn clear_channel(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    // TODO(engineer): unmigrated for the same reason as link_channel (no use case yet).
    let found = require_owner(&state, &commission_id, &actor_id).await?;

    let Some(previous) = found.linked_channel else {
        return Ok(StatusCode::NO_CONTENT.into_response());
    };

    let entry = NewChangelogEntry::event(
        commission_id,
        ChangelogEntryKind::ChannelUnlinked,
        actor_id,
        json!({ "channel": previous.as_str() }),
        Utc::now(),
    );
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            let changed = uow
                .commissions()
                .set_linked_channel(&commission_id, None)
                .await?;
            if changed {
                uow.changelog().append(&entry).await?;
            }
            Ok(())
        })
        .await?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
