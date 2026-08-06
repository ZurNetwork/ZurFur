//! `POST /commissions/{id}/elements` and
//! `DELETE /commissions/{id}/elements/{element}` — the owner composes the
//! commission (ZMVP-166; Flat Composition DD `45514754`).
//!
//! **One pair of routes for all of composition**, where the tree needed three
//! (`/surfaces`, `/components`, `/nodes/{node}`). That collapse is the model's,
//! not a simplification: surfaces and tabs are a code-declared skeleton, so
//! there is nothing to create or delete for them, and the only thing a caller
//! ever writes is an element. There is likewise no "cannot remove" arm — no
//! element id addresses a skeleton part.
//!
//! The add accepts an **address** (`tab` + `surface`), a `type`, and an opaque
//! `payload` — nothing else. There is no mode (every element is born `Total`;
//! widening is ZMVP-74's explicit act), no position (the store assigns append
//! order within the band, on the transaction), and no band (the vocabulary is
//! undecided pending ZMVP-171 — everything lands in the placeholder). Element
//! edits append **no** changelog entry: they are not in the frozen entry
//! taxonomy (ZMVP-87).
//!
//! ⚠️ **The tab id has no read route yet.** A caller learns a tab's id from the
//! composition `GET` that ZMVP-163 mints; until then nothing over HTTP hands one
//! out, so these routes are exercised through ids read from the store. The
//! address is by id deliberately (the DD's "elements address surfaces by id"),
//! not by name.

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{
        CommissionId, ElementId, ElementPayload, ElementType, NewElement, SurfaceAddress,
        SurfaceName, TabId,
    },
    ports::{ElementNotFound, UnitOfWork, UnknownSurface, UnknownTab},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// The `POST /commissions/{id}/elements` request body: where the element goes
/// (`tab` by id, `surface` by declared name), what it is (`type`), and its
/// opaque payload — any JSON value, carried verbatim; omitted, it defaults to
/// the empty object (the column's own default). Mode, band, and position are
/// core-assigned and the creator is the session.
///
/// ⚠️ contract-decision-needed: `payload` is an unschematized `serde_json::Value`
/// passthrough — v1's generic, untyped element contract (no type catalog yet,
/// per the Flat Composition DD). Whatever shape a client sends round-trips
/// verbatim, with no schema behind it; resolution tracks `VERSIONING.md` §8 Q9
/// (Engineer-deferred) and the catalog ticket ZMVP-171.
#[derive(Deserialize)]
pub(super) struct AddElementBody {
    tab: Uuid,
    surface: String,
    r#type: String,
    #[serde(default = "empty_object")]
    payload: serde_json::Value,
}

/// `POST /commissions/{id}/elements`'s `201` body: the new element's id — see
/// [`add_element`].
#[derive(Serialize)]
struct AddElementResponse {
    id: Uuid,
}

/// The default payload for a request that omits it — [`ElementPayload`]'s own
/// empty value (`{}`), so the omitted-body default, the domain default, and the
/// `commission_element.payload` column default are one decision spelled once.
fn empty_object() -> serde_json::Value {
    ElementPayload::default().into_value()
}

/// Contribute an element into one of the commission's declared surfaces
/// (ZMVP-166), as its owner.
///
/// Owner-only via the shared [`require_owner`] gate: a non-participant — and a
/// truly absent commission — gets the uniform
/// [`commission_not_found`](Problem::commission_not_found) 404 (never a 403; no
/// existence oracle). A tab that doesn't exist in **this** commission —
/// fabricated, or belonging to some other commission — is refused by the store
/// as one indistinguishable [`UnknownTab`], answered
/// [`tab_not_found`](Problem::tab_not_found). A **(tab, surface) pair** the code
/// skeleton does not declare — an invented surface name, or a real one addressed
/// under a tab that does not hold it — is [`UnknownSurface`], answered
/// [`unknown_surface`](Problem::unknown_surface) — a `422`, not a 404, because
/// the surface vocabulary is global and invariant, so refusing one leaks nothing
/// about anybody's commission. A malformed body, or a `surface`/`type` that
/// isn't a well-formed label, is a `422`.
///
/// The insert runs in one unit of work; order is assigned there (append = max +
/// 1 within the band, on the transaction). Returns `201 Created` with the new
/// element's id — `{"id": "…"}` — the identity the removal route addresses it
/// by.
pub(super) async fn add_element(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<AddElementBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let element_type = ElementType::try_from(body.r#type)
        .map_err(|err| Problem::invalid_request(format!("Invalid element type: {err}.")))?;
    let address = address(body.tab, body.surface)?;

    // The boundary wrap: untrusted JSON becomes an `ElementPayload` here, once,
    // and travels non-serializable from this line on (ZMVP-170 owns the only
    // path back out).
    let payload = ElementPayload::from(body.payload);
    let element = NewElement::contributed(
        commission,
        address,
        element_type,
        payload,
        user.id,
        Utc::now(),
    );
    let element_id = *element.id;

    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().add_element(&element).await
        })
        .await
        .map_err(to_problem)?;

    let body = AddElementResponse { id: element_id };
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

/// Remove an element from the commission's composition (ZMVP-166), as its owner.
///
/// Owner-only via the shared [`require_owner`] gate: a non-participant — and a
/// truly absent commission — gets the uniform
/// [`commission_not_found`](Problem::commission_not_found) 404 (never a 403; no
/// existence oracle). An element that doesn't exist in **this** commission —
/// fabricated, or belonging to some other commission — is refused by the store
/// as one indistinguishable [`ElementNotFound`], answered
/// [`element_not_found`](Problem::element_not_found). There is no `409` arm at
/// all: every element is removable, because the skeleton parts a caller might
/// have tried to remove are not elements. The removal and the ordering group's
/// renumbering run in one unit of work. Returns `204 No Content` — there is
/// nothing to say about what no longer exists.
pub(super) async fn remove_element(
    State(state): State<AppState>,
    Path((id, element)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let element = ElementId::new(element);
    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().remove_element(commission, element).await
        })
        .await
        .map_err(to_problem)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Parse a request body's `tab` + `surface` into a [`SurfaceAddress`] — the ONE
/// address-parsing path, shared by every route that writes an element (here,
/// [`slots`](super::slots), [`seats`](super::seats)), so the three cannot drift
/// on how an address is built or on what a malformed one answers.
///
/// The bodies keep their own `tab`/`surface` fields (each route's request shape
/// is its own contract); only the *interpretation* is shared. A `surface` that
/// isn't a well-formed label is a `422` here — whether the skeleton **declares**
/// the (tab, surface) pair is the store's question, answered with
/// [`UnknownSurface`] and mapped by [`to_problem`].
pub(super) fn address(tab: Uuid, surface: String) -> Result<SurfaceAddress, Problem> {
    let surface = SurfaceName::try_from(surface)
        .map_err(|err| Problem::invalid_request(format!("Invalid surface: {err}.")))?;
    Ok(SurfaceAddress::new(TabId::new(tab), surface))
}

/// The store's composition errors as RFC 9457 problems — the ONE mapping, shared
/// by every route that writes an element (here, [`slots`](super::slots),
/// [`seats`](super::seats)), so a caller cannot get one answer for an unknown tab
/// on the generic add and a different one on a Slot declaration.
pub(super) fn to_problem(err: anyhow::Error) -> Problem {
    if err.downcast_ref::<UnknownTab>().is_some() {
        Problem::tab_not_found()
    } else if err.downcast_ref::<UnknownSurface>().is_some() {
        Problem::unknown_surface()
    } else if err.downcast_ref::<ElementNotFound>().is_some() {
        Problem::element_not_found()
    } else {
        err.into()
    }
}

#[cfg(test)]
mod tests {
    //! Pins the `201` body's wire shape: `{"id": "<uuid>"}` — the same bare-id
    //! object the retired surface/component adds emitted, so the collapse to one
    //! route moved no bytes for the field a client reads (ZMVP-158 AC1/AC3).

    use super::*;

    #[test]
    fn add_element_response_serializes_to_a_bare_id_object() {
        let id = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000001").unwrap();
        let body = AddElementResponse { id };

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            format!("{{\"id\":\"{id}\"}}")
        );
    }
}
