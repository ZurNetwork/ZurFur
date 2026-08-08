//! `GET /commissions/{id}` — one commission with the **viewer's projection** of
//! its flat composition (ZMVP-163; Flat Composition DD `45514754`, wire half of
//! DD `42762241`'s surviving decisions).
//!
//! This is the only route through which composition content leaves the process,
//! and the shape of that exit is enforced by types rather than by care:
//! [`CommissionComposition`](domain::elements::commission::CommissionComposition)
//! carries no `serde::Serialize`, so the loaded aggregate cannot be answered
//! with; the payload inside it carries none either, so no caller can reach past
//! the aggregate and pick one up. What this handler serializes is a
//! [`ProjectedComposition`](domain::elements::commission::ProjectedComposition),
//! which exists only as the output of
//! [`project`](domain::elements::commission::CommissionComposition::project) —
//! `min(tab, surface, element)` already applied, under the tier the viewer
//! stands at.
//!
//! **The tier v1 serves is [`ViewerTier::PARTICIPANT`]**, and only that one. The
//! closed door is upstream of the projection and unchanged: [`require_participant`]
//! answers everyone else — including a caller naming a commission that does not
//! exist — with the one uniform 404, so this endpoint is not an existence
//! oracle. Mapping a *non*-participant onto a tier from the commission's
//! `visibility` (and later a seat's ceiling) is ZMVP-75's lane, reserved here by
//! `composition_withheld` and by the projection taking a tier at all rather than
//! a boolean.
//!
//! The response is the contract's generated `GetCommissionResponse` (DD
//! `40992770`): lowerCamelCase keys (R1), absent optionals omit their keys (R4),
//! ids opaque (R6), vocabularies as strings (R8) — and **no ordinal anywhere**
//! (R3), because a sparse `position` would count what the projection removed.
//!
//! ⚠️ **One honest limit on what omitting a field buys.** Element and tab ids
//! are UUIDv7, so they carry an embedded creation timestamp: leaving
//! `created_at` off an element withholds the field, not the fact. R6 binds
//! clients not to parse ids, but it is a contract, not a defence — the same is
//! already true of every commission and account id this API serves, so the
//! omission is consistency and one-way-door discipline (adding a field later is
//! additive) rather than a correlation guarantee. If element timing ever needs
//! to be genuinely unavailable to a viewer, the id is what has to change.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::elements::commission::{
    CommissionId, ProjectedComposition, ProjectedElement, ProjectedSurface, ProjectedTab,
    ViewerTier,
};
use tower_sessions::Session;
use uuid::Uuid;

use super::{list::wire_commission, require_participant};
use crate::generated::{
    CommissionElement, CommissionSurface, CommissionTab, GetCommissionResponse, commission_element,
};
use crate::{AppState, problem::Problem};

/// Render one projected tab into the contract's message.
fn wire_tab(tab: ProjectedTab) -> CommissionTab {
    CommissionTab {
        id: tab.id.to_string(),
        tab: tab.tab.as_str().to_owned(),
        mode: tab.mode.as_str().to_owned(),
    }
}

/// Render one projected surface. The tab is cited **by id** — the wire mirrors
/// the model's addressing, so nothing on either side owns a parent pointer.
fn wire_surface(surface: ProjectedSurface) -> CommissionSurface {
    CommissionSurface {
        surface: surface.surface.as_str().to_owned(),
        tab_id: surface.tab.to_string(),
        mode: surface.mode.as_str().to_owned(),
    }
}

/// Render one projected element.
///
/// `payload` is **always set** — the projection renders every payload to text,
/// `"{}"` included — so the `oneof` never arrives unset and absence never has to
/// mean anything (R4). It travels as a JSON *string* rather than as structure:
/// `google.protobuf.Struct` floats every integer and dies above 2^53 on this
/// tier while silently truncating on the other (DD `42762241` D4).
///
/// The element's own `mode` is served, not its effective one — the three terms
/// travel at their own grain (Engineer, 2026-08-04) and the min has already been
/// applied to decide that this element is here at all. Neither `created_by` nor
/// a `position` is rendered: there is no field to render them into, deliberately.
fn wire_element(element: ProjectedElement) -> CommissionElement {
    let payload = commission_element::Payload::OpaqueJson(element.payload_json);
    CommissionElement {
        id: element.id.to_string(),
        tab_id: element.tab.to_string(),
        surface: element.surface.as_str().to_owned(),
        kind: element.kind.as_str().to_owned(),
        mode: element.mode.as_str().to_owned(),
        payload: Some(payload),
    }
}

/// Fold a whole projection into the response's three repeated fields.
///
/// Takes the [`ProjectedComposition`] and nothing else on purpose: this function
/// has no access to a composition that has not been projected, so there is no
/// version of it that could serialize one.
fn wire_composition(
    commission: domain::elements::commission::Commission,
    composition: ProjectedComposition,
) -> GetCommissionResponse {
    let envelope = wire_commission(commission);
    GetCommissionResponse {
        id: envelope.id,
        title: envelope.title,
        lifecycle: envelope.lifecycle,
        visibility: envelope.visibility,
        deadline: envelope.deadline,
        maturity: envelope.maturity,
        direction_status: envelope.direction_status,
        deadline_status: envelope.deadline_status,
        linked_channel: envelope.linked_channel,
        created_at: envelope.created_at,
        // Stated, never inferred (R4). A participant is never withheld from, so
        // v1 always says `false`; the field is minted now because ZMVP-75 is
        // exactly where an empty `elements` would otherwise start meaning two
        // things at once.
        composition_withheld: false,
        tabs: composition.tabs.into_iter().map(wire_tab).collect(),
        surfaces: composition.surfaces.into_iter().map(wire_surface).collect(),
        elements: composition.elements.into_iter().map(wire_element).collect(),
    }
}

/// One commission and the composition its viewer may see (ZMVP-163).
///
/// Participant-gated through the shared [`require_participant`] gate: a
/// non-participant and an absent id are answered identically, with the uniform
/// [`commission_not_found`](Problem::commission_not_found) 404 — never a 403,
/// which would confirm existence. A signed-out caller gets a `401`.
///
/// A commission that resolves but whose composition does not load has lost the
/// tab rows minted with it. That is corruption, not a state, and it answers the
/// closed door — the same 404 — rather than a partial commission that quietly
/// omits everything.
///
/// Outcomes:
/// - `200 { "id", "title", "lifecycle", "visibility", "deadline"?, "maturity"?,
///   "directionStatus"?, "deadlineStatus"?, "linkedChannel"?, "createdAt",
///   "compositionWithheld"?, "tabs": [...], "surfaces": [...],
///   "elements": [...] }` — absent optionals omit their keys; `false` booleans
///   omit theirs (canonical ProtoJSON)
/// - `401` — not signed in
/// - `404` — not a participant, or no such commission (one body for both)
pub(super) async fn get_commission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_participant(&state, commission, user.id).await?;

    let found = state
        .commissions
        .find(commission)
        .await?
        .ok_or_else(Problem::commission_not_found)?;
    let loaded = state
        .commissions
        .load_composition(commission)
        .await?
        .ok_or_else(Problem::commission_not_found)?;

    // The one tier v1 serves. Reaching the wire without this call is not
    // possible: `wire_composition` takes only what `project` returns.
    let projected = loaded.project(ViewerTier::PARTICIPANT);

    let body = wire_composition(found, projected);
    let response = (StatusCode::OK, Json(body)).into_response();
    Ok(response)
}
