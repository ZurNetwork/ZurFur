//! `GET /commissions/{id}/changelog` — a Participant reads the commission's
//! stream in order (ZMVP-87 AC5). Read-only by design: the changelog's HTTP
//! surface has no other method (append happens as a side of domain acts; AC4).

use application::commission::changelog::read;
use axum::{
    Json,
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use domain::elements::commission::CommissionId;
use serde::Serialize;

use crate::{AppState, extract::CallingUser, problem::Problem, wire_time::WireTimestamp};

/// One changelog entry as the API serves it: the stored envelope, kind as its
/// stable token, actor as a bare id (`null` = a system entry), `seq` as the
/// ascending ordering key.
///
/// ⚠️ contract-decision-needed: `payload` is an unschematized
/// `serde_json::Value` passthrough (tracks `VERSIONING.md` §8 Q9).
#[derive(Serialize)]
struct ChangelogEntryBody {
    seq: i64,
    kind: &'static str,
    actor_id: Option<String>,
    payload: serde_json::Value,
    note: Option<String>,
    created_at: WireTimestamp,
}

/// Reads the commission's changelog in stream order: a bare JSON array of
/// entries, ascending `seq`. Participant-only; `404` otherwise. Unpaginated.
pub(super) async fn read_changelog(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    let query = read::Query {
        actor_id,
        commission_id,
    };

    let entries = state.app().commissions().changelog().read(query).await?;

    let result: Vec<ChangelogEntryBody> = entries
        .entries
        .into_iter()
        .map(|cl| ChangelogEntryBody {
            seq: cl.seq,
            kind: cl.kind.as_str(),
            actor_id: cl.actor_id.map(|a| a.to_string()),
            payload: cl.payload,
            note: cl.note,
            created_at: WireTimestamp::from(cl.created_at),
        })
        .collect();

    Ok(Json(result).into_response())
}

#[cfg(test)]
mod tests {
    //! Pins `ChangelogEntryBody.created_at`'s wire format.

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
