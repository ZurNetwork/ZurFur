//! `POST /commissions/{id}/seats` — the owner declares a Seat on the
//! commission (ZMVP-76; Referenceable/Slot/Seat DD `28311564` Decisions 1, 3,
//! 8): a 1:1 structural participant position, born **vacant**, typed by an
//! open kind (Creator, Client, … — deliberately not the Role enum: Role keeps
//! authority, aliases keep display), optionally carrying its requirements —
//! the v1 vocabulary of a free-text prompt and/or an external link (no form
//! builder; that is a Plugin).
//!
//! A dedicated endpoint rather than the generic component add: a seat is a
//! component in the tree (the untyped v1 contract gives it position and
//! visibility inheritance) **plus** the typed satellite the core interprets,
//! and only a dedicated route can populate both atomically. The declaration is
//! changelog-recorded (`seat_declared` — an existing variant of ZMVP-87's
//! frozen taxonomy) in the same unit of work.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{
        ChangelogEntryKind, CommissionId, NewChangelogEntry, NewSeat, NodeId, SeatKind, SeatLink,
        SeatPrompt,
    },
    ports::{ParentNodeNotFound, ParentNotASurface, transaction},
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// The `POST /commissions/{id}/seats` request body: the existing **surface** to
/// declare the seat under (the seat inherits its visibility — a vacant seat
/// under a Description-visible surface is the published ask), the seat's typed
/// `kind` (required; open vocabulary), and the optional requirements — a
/// free-text `prompt` and/or an external `link`, each validated at the
/// boundary. There is deliberately no occupant field: seats are born vacant
/// (filling one is ZMVP-79's invitation-mediated act).
#[derive(Deserialize)]
pub(super) struct DeclareSeatBody {
    parent: Uuid,
    kind: String,
    prompt: Option<String>,
    link: Option<String>,
}

/// Declare a Seat under an existing **surface** of the commission's tree
/// (ZMVP-76 AC1/AC2), as its owner.
///
/// Owner-only via the shared [`require_owner`] gate (the one managing-authority
/// path; ZMVP-83 activates its Admin arm): a non-participant — and a truly
/// absent commission — gets the uniform
/// [`commission_not_found`](Problem::commission_not_found) 404 (never a 403; no
/// existence oracle). A malformed body, a blank/oversized kind, or an invalid
/// prompt/link is a `422`. A parent node that doesn't exist in **this**
/// commission's tree — fabricated, or belonging to some other commission — is
/// refused by the store as one indistinguishable [`ParentNodeNotFound`],
/// answered [`node_not_found`](Problem::node_not_found); a parent that exists
/// here but is a component is [`ParentNotASurface`], answered with the honest
/// `409` [`parent_not_a_surface`](Problem::parent_not_a_surface). The seat's
/// node, its satellite, and its `seat_declared` changelog entry land in **one
/// unit of work** — a seat can never exist without its record. Returns `201
/// Created` with the seat's node id — `{"id": "…"}` — the identity later
/// tickets (invitations 78, applications 80, ceilings 96) address it by.
pub(super) async fn declare_seat(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<DeclareSeatBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let kind = SeatKind::try_new(body.kind).map_err(|e| Problem::invalid_request(e.to_string()))?;
    let prompt = body
        .prompt
        .map(SeatPrompt::try_new)
        .transpose()
        .map_err(|e| Problem::invalid_request(e.to_string()))?;
    let link = body
        .link
        .map(SeatLink::try_new)
        .transpose()
        .map_err(|e| Problem::invalid_request(e.to_string()))?;

    let now = Utc::now();
    let seat = NewSeat::under(
        commission,
        NodeId::new(body.parent),
        kind,
        prompt,
        link,
        user.id,
        now,
    );
    let seat_id = *seat.id;
    // The record: the payload carries the kind so the sentence ("declared a
    // Creator seat") renders without joins (the DD's core-renderable rule);
    // the seat's node id names which seat for later entries in the stream.
    let entry = NewChangelogEntry::event(
        commission,
        ChangelogEntryKind::SeatDeclared,
        user.id,
        json!({ "kind": seat.kind.as_str(), "seat": seat_id }),
        now,
    );

    transaction(&*state.database, |uow| {
        Box::pin(async move {
            uow.commissions().declare_seat(&seat).await?;
            uow.changelog().append(&entry).await
        })
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

    Ok((StatusCode::CREATED, Json(json!({ "id": seat_id }))).into_response())
}
