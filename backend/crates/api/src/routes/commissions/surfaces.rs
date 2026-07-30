//! `POST /commissions/{id}/surfaces` — the owner grows the commission's content
//! tree with a new surface (ZMVP-71; Surfaces DD `28246028`, Tree Storage DD
//! `28409880`).
//!
//! The route accepts only the parent to grow under: **no mode** (every new
//! surface is born `Total` — widening is ZMVP-74's explicit act) and **no
//! payload** (a surface is grouping/layout; typed content arrives as Components,
//! ZMVP-72). Tree edits append **no** changelog entry — they are not in the
//! frozen entry taxonomy (ZMVP-87).

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{CommissionId, NewSurface, NodeId},
    ports::{ParentNodeNotFound, ParentNotASurface, TreeDepthExceeded, UnitOfWork},
};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// The `POST /commissions/{id}/surfaces` request body: the existing surface to
/// grow under. Nothing else is accepted from the client — mode and position are
/// core-assigned (`Total`, append order), and the creator is the session.
#[derive(Deserialize)]
pub(super) struct AddSurfaceBody {
    parent: Uuid,
}

/// `POST /commissions/{id}/surfaces`'s `201` body: the new node's id — see
/// [`add_surface`].
#[derive(Serialize)]
struct AddSurfaceResponse {
    id: Uuid,
}

/// Add a surface under an existing surface of the commission's tree (ZMVP-71
/// AC2), as its owner.
///
/// Owner-only via the shared [`require_owner`] gate: a non-participant — and a
/// truly absent commission — gets the uniform
/// [`commission_not_found`](Problem::commission_not_found) 404 (never a 403; no
/// existence oracle). A parent node that doesn't exist in **this** commission's
/// tree — fabricated, or belonging to some other commission — is refused by the
/// store as one indistinguishable [`ParentNodeNotFound`], answered
/// [`node_not_found`](Problem::node_not_found); a parent that exists here but
/// is a component is [`ParentNotASurface`], answered
/// [`parent_not_a_surface`](Problem::parent_not_a_surface) (ZMVP-72:
/// components never have children); a parent already at the write-side depth
/// cap is [`TreeDepthExceeded`], answered
/// [`tree_depth_exceeded`](Problem::tree_depth_exceeded) (ZMVP-164); a
/// malformed body is a `422`.
/// The insert runs in one unit of work; sibling order is assigned there
/// (append = max + 1, on the transaction). Returns `201 Created` with the new
/// node's id — `{"id": "…"}` — since no tree read exposes ids until the
/// projection lands (ZMVP-75).
pub(super) async fn add_surface(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<AddSurfaceBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let surface = NewSurface::under(commission, NodeId::new(body.parent), user.id, Utc::now());
    let node_id = *surface.id;

    state
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().add_surface(&surface).await
        })
        .await
        .map_err(|err| {
            if err.downcast_ref::<ParentNodeNotFound>().is_some() {
                Problem::node_not_found()
            } else if err.downcast_ref::<ParentNotASurface>().is_some() {
                Problem::parent_not_a_surface()
            } else if err.downcast_ref::<TreeDepthExceeded>().is_some() {
                Problem::tree_depth_exceeded()
            } else {
                err.into()
            }
        })?;

    let body = AddSurfaceResponse { id: node_id };
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

#[cfg(test)]
mod tests {
    //! Pins the `201` body's wire shape: `{"id": "<uuid>"}` — the exact string
    //! form `json!({ "id": node_id })` (a bare `Uuid`) used to emit, so swapping
    //! in the named struct moved no bytes (ZMVP-158 AC1/AC3).

    use super::*;

    #[test]
    fn add_surface_response_serializes_to_a_bare_id_object() {
        let id = Uuid::parse_str("0192f6f0-0000-7000-8000-000000000001").unwrap();
        let body = AddSurfaceResponse { id };

        assert_eq!(
            serde_json::to_string(&body).unwrap(),
            format!("{{\"id\":\"{id}\"}}")
        );
    }
}
