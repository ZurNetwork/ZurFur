//! `POST /commissions/{id}/elements` and
//! `DELETE /commissions/{id}/elements/{element}` — the owner composes the
//! commission out of flat typed elements placed into declared surfaces and
//! tabs. ⚠️ The tab id has no read route yet.

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
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, extract::CallingUser, problem::Problem};

/// The `POST /commissions/{id}/elements` request body: where the element goes
/// (`tab` by id, `surface` by declared name), what it is (`type`), and its
/// opaque payload (defaults to empty if omitted; schema undecided, VERSIONING.md §8 Q9).
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

/// The default payload for a request that omits it.
fn empty_object() -> serde_json::Value {
    ElementPayload::default().into_value()
}

/// Contributes an element into one of the commission's declared surfaces, as
/// its owner. Owner-only; `404` for a non-participant, `404 tab_not_found`
/// for an unknown tab, `422 unknown_surface` for an undeclared (tab, surface)
/// pair, `422` for a malformed body. Returns `201 Created` with `{"id": "…"}`.
pub(super) async fn add_element(
    State(state): State<AppState>,
    Path(commission_id): Path<CommissionId>,
    CallingUser(actor_id): CallingUser,
    body: Result<Json<AddElementBody>, JsonRejection>,
) -> Result<Response, Problem> {
    // TODO(engineer): no use case exists yet, so this still authorizes and
    // transacts in the driver (DD 55836674 D6/D7 place both in the application layer).
    require_owner(&state, &commission_id, &actor_id).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let element_type = ElementType::try_from(body.r#type)
        .map_err(|err| Problem::invalid_request(format!("Invalid element type: {err}.")))?;
    let address = address(body.tab, body.surface)?;

    let payload = ElementPayload::from(body.payload);
    let element = NewElement::contributed(
        commission_id,
        address,
        element_type,
        payload,
        actor_id,
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

/// Removes an element from the commission's composition, as its owner.
/// Owner-only; `404 commission_not_found` for a non-participant, `404
/// element_not_found` for an element not in this commission. Returns `204
/// No Content`.
pub(super) async fn remove_element(
    State(state): State<AppState>,
    Path((commission_id, element)): Path<(CommissionId, ElementId)>,
    CallingUser(actor_id): CallingUser,
) -> Result<Response, Problem> {
    // TODO(engineer): unmigrated for the same reason as add_element (no use case yet).
    require_owner(&state, &commission_id, &actor_id).await?;

    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions()
                .remove_element(&commission_id, &element)
                .await
        })
        .await
        .map_err(to_problem)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

/// Parses a request body's `tab` + `surface` into a [`SurfaceAddress`] — the
/// one address-parsing path shared by every route that writes an element.
/// `422` for a malformed `surface`.
pub(super) fn address(tab: Uuid, surface: String) -> Result<SurfaceAddress, Problem> {
    let surface = SurfaceName::try_from(surface)
        .map_err(|err| Problem::invalid_request(format!("Invalid surface: {err}.")))?;
    Ok(SurfaceAddress::new(TabId::new(tab), surface))
}

/// Maps the store's composition errors to RFC 9457 problems — the one
/// mapping shared by every route that writes an element.
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
    //! Pins the `201` body's wire shape: `{"id": "<uuid>"}`.

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
