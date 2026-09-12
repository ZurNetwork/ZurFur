//! `POST /commissions/{id}/slots` — the owner declares a batch of Character
//! Slots: positions with a required title and optional notes.
//! The body is an array; the batch lands all-or-nothing. No fill
//! surface exists here — an empty Slot is a valid, permanent state.

use application::commission::slots::declare::{self, SlotBody};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::commission::{CommissionId, SlotTitle, TabId};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, extract::CallingUser, problem::Problem};

/// `POST /commissions/{id}/slots`'s `201` body: the id of each newly declared
/// slot's carrying element, in request order — see [`declare_slots`].
#[derive(Serialize)]
struct DeclareSlotsResponse {
    ids: Vec<Uuid>,
}

/// One Slot of the `POST /commissions/{id}/slots` request body (a JSON array
/// of these): the address (`tab` + `surface`), the required title, and
/// optional notes. No occupant/character field.
#[derive(Deserialize)]
pub(super) struct DeclareSlotBody {
    tab: Uuid,
    surface: String,
    title: String,
    #[serde(default)]
    notes: Option<String>,
}

/// Declares a batch of Slots, as the commission's owner. All-or-nothing.
/// Owner-only; `422` for an empty array, an invalid title, or a bad address
/// (`404 tab_not_found` / `422 unknown_surface`). Returns `201 Created` with
/// `{"ids": ["…", …]}` in request order.
pub(super) async fn declare_slots(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(user_id): CallingUser,
    body: Result<Json<Vec<DeclareSlotBody>>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    if body.is_empty() {
        return Err(Problem::invalid_request(
            "Declare at least one slot: the body is an array of slot objects.",
        ));
    }

    let all_slots: Vec<SlotBody> =
        body.into_iter()
            .map(|entry| {
                let title = entry.title.parse::<SlotTitle>().map_err(|err| {
                    Problem::invalid_request(format!("Invalid slot title: {err}."))
                })?;
                let surface = super::elements::address(entry.tab, entry.surface)?;
                let slot = SlotBody {
                    tab: TabId::new(entry.tab),
                    surface,
                    title,
                    notes: entry.notes,
                };
                Ok(slot)
            })
            .collect::<Result<_, Problem>>()?;

    let command = declare::Command {
        commission_id,
        slots: all_slots,
        user_id,
    };

    let declared = state
        .app()
        .commissions()
        .slots()
        .declare(command, Utc::now())
        .await?;
    let ids: Vec<Uuid> = declared.slot_ids.into_iter().map(|slot| *slot).collect();

    let body = DeclareSlotsResponse { ids };
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

#[cfg(test)]
mod tests {
    //! Pins the `201` body's wire shape: `{"ids": ["<uuid>", …]}`.

    use super::*;

    #[test]
    fn declare_slots_response_serializes_to_a_bare_ids_array() {
        let first = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000004").unwrap();
        let second = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000005").unwrap();
        let body = DeclareSlotsResponse {
            ids: vec![first, second],
        };

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            format!("{{\"ids\":[\"{first}\",\"{second}\"]}}")
        );
    }
}
