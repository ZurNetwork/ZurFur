//! `DELETE /commissions/{id}/nodes/{node}` — the owner prunes the commission's
//! content tree (ZMVP-73; Surfaces DD `28246028`, Tree Storage DD `28409880`).
//!
//! Removal is **subtree-deep** and kind-agnostic: one route for surfaces and
//! components alike, because a node's removal semantics follow from its shape —
//! a surface takes its entire subtree with it, a component (a leaf) goes
//! singly. The fixed skeleton is protected: the root surface refuses with a
//! `409` (`cannot_remove_root`), and the Title is not a tree node at all, so no
//! node id can even address it. The remaining siblings renumber in the same
//! transaction, so positions stay consistent. Tree edits append **no**
//! changelog entry — they are not in the frozen entry taxonomy (ZMVP-87).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{
    elements::commission::{CommissionId, NodeId},
    ports::{CannotRemoveRoot, NodeNotFound, transaction},
};
use tower_sessions::Session;
use uuid::Uuid;

use super::require_owner;
use crate::{AppState, problem::Problem};

/// Remove a node — and, with a surface, its entire subtree — from the
/// commission's tree (ZMVP-73 AC1/AC2), as its owner.
///
/// Owner-only via the shared [`require_owner`] gate: a non-participant — and a
/// truly absent commission — gets the uniform
/// [`commission_not_found`](Problem::commission_not_found) 404 (never a 403; no
/// existence oracle). A node that doesn't exist in **this** commission's tree —
/// fabricated, or belonging to some other commission — is refused by the store
/// as one indistinguishable [`NodeNotFound`], answered
/// [`node_not_found`](Problem::node_not_found) (a foreign *root* included: the
/// root check runs only past that gate); the commission's own root surface is
/// [`CannotRemoveRoot`], answered with the honest `409`
/// [`cannot_remove_root`](Problem::cannot_remove_root) (AC3). The prune and the
/// sibling renumbering run in one unit of work. Returns `204 No Content` —
/// there is nothing to say about what no longer exists.
pub(super) async fn remove_node(
    State(state): State<AppState>,
    Path((id, node)): Path<(Uuid, Uuid)>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    require_owner(&state, commission, &user).await?;

    let node = NodeId::new(node);
    transaction(&*state.database, |uow| {
        Box::pin(async move { uow.commissions().remove_node(commission, node).await })
    })
    .await
    .map_err(|err| {
        if err.downcast_ref::<NodeNotFound>().is_some() {
            Problem::node_not_found()
        } else if err.downcast_ref::<CannotRemoveRoot>().is_some() {
            Problem::cannot_remove_root()
        } else {
            err.into()
        }
    })?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
