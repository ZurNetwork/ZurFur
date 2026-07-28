//! `GET /commissions/{id}/changelog` — a Participant reads the commission's
//! stream in order (ZMVP-87 AC5). Read-only by design: the changelog's HTTP
//! surface has no other method (append happens as a side of domain acts; AC4).

use axum::{
    Json,
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use domain::elements::commission::CommissionId;
use serde::Serialize;
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem, wire_time::WireTimestamp};

/// One changelog entry as the API serves it — the stored envelope, with the
/// kind as its stable token and the actor as a bare id (`null` = a system
/// entry). `seq` is the explicit ordering key (ascending = stream order);
/// `created_at` is carried for display.
///
/// `created_at` is a [`WireTimestamp`] — canonical ProtoJSON, RFC 3339,
/// **Z-normalized**, 0/3/6/9 fractional digits, range-validated to years
/// 0001–9999 — byte-identical to what chrono's serde previously emitted for
/// every in-range instant (verified empirically; ZMVP-158 AC5), so adopting it
/// here does not move the wire. Serialization is bounded to that same
/// 0001–9999 range: a stored `created_at` outside it fails the whole list's
/// response (matching the accepted behavior on the `/api/v1` corpus routes
/// this type already serves) — unreachable in practice, since `created_at` is
/// DB-minted `now()`, never client-supplied.
///
/// ⚠️ contract-decision-needed: `payload` is an unschematized `serde_json::Value`
/// passthrough — the hole through which every changelog-payload `json!` site
/// (14 of them, out of this ticket's scope) reaches the client verbatim, with
/// no schema behind any of them. Resolution tracks `VERSIONING.md` §8 Q9
/// (Engineer-deferred). One of those payloads, `markup_added`, itself carries a
/// `Markup` value out this same hole — the other facet of the same
/// unschematized passthrough is `Json<Markup>` on the way *in*
/// (`routes/commissions/markup.rs`, `add_markup`).
#[derive(Serialize)]
struct ChangelogEntryBody {
    seq: i64,
    kind: &'static str,
    actor_id: Option<Uuid>,
    payload: serde_json::Value,
    note: Option<String>,
    created_at: WireTimestamp,
}

/// Read the commission's changelog, in stream order (ZMVP-87 AC5): a bare JSON
/// array of entries, ascending `seq`. Participant-only behind
/// [`require_participant`](super::require_participant) — a non-participant (or
/// an absent commission) gets the uniform `commission_not_found` 404, never a
/// 403. Unpaginated at this ticket; cursors are ZMVP-100's job.
pub(super) async fn read_changelog(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_participant(&state, commission, user.id).await?;

    let entries: Vec<ChangelogEntryBody> = state
        .changelog
        .entries(commission)
        .await?
        .into_iter()
        .map(|entry| ChangelogEntryBody {
            seq: entry.seq,
            kind: entry.kind.as_str(),
            actor_id: entry.actor_id.map(|actor| *actor),
            payload: entry.payload,
            note: entry.note,
            created_at: WireTimestamp::from(entry.created_at),
        })
        .collect();

    Ok(Json(entries).into_response())
}

#[cfg(test)]
mod tests {
    //! Pins `ChangelogEntryBody.created_at`'s wire format: canonical ProtoJSON
    //! (RFC 3339, Z-normalized) — see `wire_time`'s own tests for the
    //! byte-identity-with-chrono pin; this one is specific to the entry body.

    use chrono::{TimeZone, Utc};

    use super::*;

    #[test]
    fn changelog_entry_body_created_at_is_z_normalized() {
        let at = Utc.with_ymd_and_hms(2025, 7, 25, 12, 0, 0).unwrap();
        let body = ChangelogEntryBody {
            seq: 1,
            kind: "created",
            actor_id: None,
            payload: serde_json::json!({}),
            note: None,
            created_at: WireTimestamp::from(at),
        };

        let wire = serde_json::to_value(&body).unwrap();
        assert_eq!(wire["created_at"], "2025-07-25T12:00:00Z");
    }
}
