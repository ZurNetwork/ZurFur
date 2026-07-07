//! `DELETE /commissions/{id}` — the owner hard-deletes a **fact-free**
//! commission (ZMVP-66; Deletion DD `3014657`: "Delete = hard delete, possible
//! only while fact-free"). Gone means gone entirely: the row and every child
//! table reap in one transaction (`commission_changelog` today, later children
//! via their own `ON DELETE CASCADE` — ruling E35), so no changelog entry
//! records the act — the record dies with the commission, by design (DD
//! retention). A fact-bearing commission is past the point of no return and is
//! refused toward Archive (ZMVP-68).

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{elements::commission::CommissionId, ports::transaction};
use tower_sessions::Session;
use uuid::Uuid;

use crate::{AppState, problem::Problem};

/// What the deleting unit of work concluded, carried out of the transaction so
/// the handler maps it to a status **after** the unit has committed. Later
/// stacks widen this at the same seam: ZMVP-88's file entries will have
/// [`Deleted`](DeleteOutcome::Deleted) carry the blob references collected
/// inside the transaction (before the row `DELETE` cascades them away), for the
/// idempotent, orphan-tolerant blob reap that runs after commit.
enum DeleteOutcome {
    /// The commission was fact-free; the row (and its cascade) is gone.
    Deleted,
    /// The commission bears facts: nothing was deleted — the caller is pointed
    /// at Archive.
    HasFacts,
}

/// Hard-delete a fact-free commission (ZMVP-66).
///
/// Owner-only via the shared [`require_owner`](super::require_owner) seam: a
/// caller who may not see the commission — including a truly absent id — gets
/// the uniform `404` (the closed door, never a `403`); a non-owner participant
/// gets an honest `403` (unreachable until ZMVP-79 seats non-owner
/// participants).
///
/// The fact gate and the delete run in **one unit of work** —
/// [`commission_has_facts`](domain::ports::CommissionWrites::commission_has_facts)
/// is asked on the same open transaction that deletes (ruling E17), so a fact
/// minted concurrently can never slip between check and delete. Fact-free →
/// the row is deleted, children cascade (ruling E35) → `204 No Content`.
/// Fact-bearing → nothing is written and the `409`
/// [`commission_has_facts`](Problem::commission_has_facts) problem points the
/// caller at Archive.
pub(super) async fn delete_commission(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    session: Session,
) -> Result<Response, Problem> {
    let user = super::current_user(&state, &session).await?;
    let commission = CommissionId::new(id);
    super::require_owner(&state, commission, &user).await?;

    let outcome = transaction(&*state.database, |uow| {
        Box::pin(async move {
            let mut commissions = uow.commissions();
            if commissions.commission_has_facts(commission).await? {
                return Ok(DeleteOutcome::HasFacts);
            }
            commissions.delete(commission).await?;
            Ok(DeleteOutcome::Deleted)
        })
    })
    .await?;

    match outcome {
        DeleteOutcome::Deleted => Ok(StatusCode::NO_CONTENT.into_response()),
        DeleteOutcome::HasFacts => Err(Problem::commission_has_facts()),
    }
}
