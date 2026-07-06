//! `POST /commissions/{id}/components` — the owner grows the commission's
//! content tree with a component, the tree's leaf (ZMVP-72; Surfaces DD
//! `28246028` amendment, Tree Storage DD `28409880`).
//!
//! The route accepts the parent **surface** to grow under and the component's
//! opaque payload — nothing else. There is no mode (a component projects with
//! its parent; none is even representable, `NodeKind::Component`) and no type
//! tag (v1 is the generic, untyped contract — the type catalog is deliberately
//! deferred). The payload is stored semantically unmodified — round-trips as an equal JSON value (jsonb is not byte-preserving); the core
//! never validates or interprets it. Tree edits append **no** changelog entry —
//! they are not in the frozen entry taxonomy (ZMVP-87).

use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use domain::{
    elements::commission::{CommissionId, NewComponent, NodeId},
    ports::{ParentNodeNotFound, ParentNotASurface, transaction},
};
use serde::Deserialize;
use serde_json::json;
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// The `POST /commissions/{id}/components` request body: the existing surface
/// to grow under, and the component's opaque payload — any JSON value, carried
/// verbatim (AC3); omitted, it defaults to the empty object (the column's own
/// default). Position is core-assigned (append order) and the creator is the
/// session.
#[derive(Deserialize)]
pub(super) struct AddComponentBody {
    parent: Uuid,
    #[serde(default = "empty_object")]
    payload: serde_json::Value,
}

/// The default payload for a request that omits it: `{}`, mirroring the
/// `commission_node.payload` column default.
fn empty_object() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
}

/// Add a component under an existing **surface** of the commission's tree
/// (ZMVP-72 AC1), as its owner.
///
/// Owner-only via the shared [`require_owner`] gate: a non-participant — and a
/// truly absent commission — gets the uniform
/// [`commission_not_found`](Problem::commission_not_found) 404 (never a 403; no
/// existence oracle). A parent node that doesn't exist in **this** commission's
/// tree — fabricated, or belonging to some other commission — is refused by the
/// store as one indistinguishable [`ParentNodeNotFound`], answered
/// [`node_not_found`](Problem::node_not_found); a parent that exists here but
/// is a component is [`ParentNotASurface`], answered with the honest `409`
/// [`parent_not_a_surface`](Problem::parent_not_a_surface) (components never
/// have children); a malformed body is a `422`. The insert runs in one unit of
/// work; sibling order is assigned there (append = max + 1, on the
/// transaction). Returns `201 Created` with the new node's id — `{"id": "…"}` —
/// since no tree read exposes ids until the projection lands (ZMVP-75).
pub(super) async fn add_component(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
    body: Result<Json<AddComponentBody>, JsonRejection>,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let Json(body) = body.map_err(|_| Problem::invalid_request("Malformed request body."))?;
    let component = NewComponent::under(
        commission,
        NodeId::new(body.parent),
        body.payload,
        user.id,
        Utc::now(),
    );
    let node_id = *component.id;

    transaction(&*state.database, |uow| {
        Box::pin(async move { uow.commissions().add_component(&component).await })
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
