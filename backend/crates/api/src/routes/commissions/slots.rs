//! `POST /commissions/{id}/slots` — the owner **declares a Slot**: a Character
//! position with a required title and optional freeform notes (ZMVP-77;
//! DESIGN/Slots `5931025`, Referenceable/Slot/Seat DD `28311564`).
//!
//! A Slot is a component in the tree whose substance lives in a satellite
//! (`commission_slot`, keyed by the slot node's id — the slot mirror of the
//! Seat satellite ruling, Gate A E20); the generic component add cannot
//! populate the satellite, hence this dedicated declaration route. **No fill
//! surface exists here or anywhere** (AC3): nothing in the request, the
//! storage, or the domain shapes can name an occupant — an empty Slot is a
//! valid, permanent state (AC2). The assignment surface arrives with the
//! Character epic. Declaring a Slot appends **no** changelog entry: the frozen
//! ZMVP-87 taxonomy carries `seat_declared` for Seats but no slot variant.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{CommissionId, NewSlot, NodeId, SlotTitle},
    ports::{ParentNodeNotFound, ParentNotASurface, transaction},
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// The `POST /commissions/{id}/slots` request body: the existing **surface** to
/// grow under, the Slot's required title, and optional freeform notes. There is
/// deliberately no occupant/character field — fill is not offered (AC3) — and
/// no payload: the slot node's payload is the empty object, its substance being
/// the satellite's.
#[derive(Deserialize)]
pub(super) struct DeclareSlotBody {
    parent: Uuid,
    title: String,
    #[serde(default)]
    notes: Option<String>,
}

/// Declare a Slot under an existing **surface** of the commission's tree
/// (ZMVP-77 AC1), as its owner.
///
/// Owner-only via the shared [`require_owner`] gate: a non-participant — and a
/// truly absent commission — gets the uniform
/// [`commission_not_found`](Problem::commission_not_found) 404 (never a 403; no
/// existence oracle). The title is validated through [`SlotTitle`] (trimmed,
/// blank refused with a `422`); notes are trimmed with blank normalizing to
/// absent. The parent walks the same gates as every tree write: absent/foreign
/// is the indistinguishable [`node_not_found`](Problem::node_not_found) 404, a
/// component parent the honest `409`
/// [`parent_not_a_surface`](Problem::parent_not_a_surface); a malformed body is
/// a `422`. Node and satellite land in one unit of work. Returns `201 Created`
/// with the new slot node's id — `{"id": "…"}` — since no tree read exposes ids
/// until the projection lands (ZMVP-75).
pub(super) async fn declare_slot(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<DeclareSlotBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let title = SlotTitle::try_new(body.title)
        .map_err(|err| Problem::invalid_request(format!("Invalid slot title: {err}.")))?;
    let notes = body
        .notes
        .as_deref()
        .map(str::trim)
        .filter(|notes| !notes.is_empty())
        .map(str::to_owned);

    let slot = NewSlot::under(
        commission,
        NodeId::new(body.parent),
        title,
        notes,
        user.id,
        Utc::now(),
    );
    let node_id = *slot.id;

    transaction(&*state.database, |uow| {
        Box::pin(async move { uow.commissions().declare_slot(&slot).await })
    })
    .await
    .map_err(|err| {
        if err.downcast_ref::<ParentNodeNotFound>().is_some() {
            Problem::node_not_found()
        } else if err.downcast_ref::<ParentNotASurface>().is_some() {
            Problem::parent_not_a_surface()
        } else {
            err.into()
        }
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "id": node_id }))).into_response())
}
