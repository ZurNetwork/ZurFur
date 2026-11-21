//! `POST /commissions/{id}/slots` — the owner declares a batch of Character
//! Slots: positions with a required title and optional notes.
//! The body is an array; the batch lands all-or-nothing. No fill
//! surface exists here — an empty Slot is a valid, permanent state.

use application::commission::slots::declare::{self, SlotBody};
use axum::{
    Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::elements::commission::SlotTitle;
use uuid::Uuid;

use crate::generated::{DeclareCommissionSlot, DeclareCommissionSlotsResponse};
use crate::{
    AppState,
    extract::{CallingUser, CommissionPath},
    problem::{FieldCode, FieldPointer, FieldRefusal, Problem},
};

/// The most Slots one declare carries: it bounds one request's unit, not a commission's count.
pub(super) const MAX_SLOTS_PER_DECLARE: usize = 32;

/// `422 invalid_request` for a body of more than [`MAX_SLOTS_PER_DECLARE`] Slots.
fn too_many_slots() -> Problem {
    Problem::invalid_request(format!(
        "Declare at most {MAX_SLOTS_PER_DECLARE} slots at once."
    ))
}

/// The title of the Slot at `index`, or its field refusal at `#/<index>/title`.
fn slot_title(index: usize, title: &str) -> Result<SlotTitle, FieldRefusal> {
    title.parse::<SlotTitle>().map_err(|rule| FieldRefusal {
        pointer: FieldPointer::item_member(index, "title"),
        code: FieldCode::from(rule),
    })
}

/// The address of one Slot: its tab, which must be a UUID, else the body is malformed, and its
/// surface.
fn slot_surface(
    entry: &DeclareCommissionSlot,
) -> Result<domain::elements::commission::SurfaceAddress, Problem> {
    let tab = entry
        .tab
        .parse::<Uuid>()
        .map_err(|_| Problem::invalid_request("Malformed request body."))?;
    super::elements::address(tab, entry.surface.clone())
}

/// Declares a batch of Slots, as the commission's owner. All-or-nothing.
///
/// `201` with `{"ids": ["…", …]}` in request order. `422` before anyone's standing: for a
/// malformed body, an empty array or more than [`MAX_SLOTS_PER_DECLARE`] Slots, and with a
/// field error at `#/<i>/title` for each title the name rule refuses. `403` for a Participant
/// who isn't the owner; `404` for anyone else and for an id that names nothing; `404
/// tab_not_found` for a tab that isn't the commission's; `422 unknown_surface` for a surface
/// the tab doesn't declare.
pub(super) async fn declare_slots(
    State(state): State<AppState>,
    CallingUser(user_id): CallingUser,
    CommissionPath(commission_id): CommissionPath,
    body: Result<Json<Vec<DeclareCommissionSlot>>, JsonRejection>,
) -> Result<Response, Problem> {
    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    if body.is_empty() {
        return Err(Problem::invalid_request(
            "Declare at least one slot: the body is an array of slot objects.",
        ));
    }
    if body.len() > MAX_SLOTS_PER_DECLARE {
        return Err(too_many_slots());
    }

    let mut titles = Vec::with_capacity(body.len());
    let mut refusals = Vec::new();
    for (index, entry) in body.iter().enumerate() {
        match slot_title(index, &entry.title) {
            Ok(title) => titles.push(title),
            Err(refusal) => refusals.push(refusal),
        }
    }
    if let Some((first, rest)) = refusals.split_first() {
        return Err(Problem::invalid_fields(first.clone(), rest.to_vec()));
    }

    let all_slots: Vec<SlotBody> = body
        .into_iter()
        .zip(titles)
        .map(|(entry, title)| {
            let surface = slot_surface(&entry)?;
            let slot = SlotBody {
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
    let ids: Vec<String> = declared
        .slot_ids
        .into_iter()
        .map(|id| id.to_string())
        .collect();

    let body = DeclareCommissionSlotsResponse { ids };
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

#[cfg(test)]
mod tests;
