use domain::elements::{commission::CommissionId, user::UserId};

use crate::commission::{CommissionError, CommissionResult, Commissions};

pub enum Outcome {
    Deleted,
    HasFacts,
}
pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}
pub struct Output {
    pub outcome: Outcome,
}

impl Commissions<'_> {
    /// Hard-deletes the commission, but only while it is fact-free. Owner-only;
    /// every other caller gets the closed door's `CommissionNotFound`. A
    /// fact-bearing commission is left untouched and reported as
    /// [`Outcome::HasFacts`] for the driver to point at Archive.
    pub async fn delete(&self, cmd: Command) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .filter(|c| c.is_owned_by(&actor_id))
            .ok_or(CommissionError::CommissionNotFound)?;

        // One unit, closed on BOTH branches. The fact gate and the delete it
        // guards must share a transaction (no TOCTOU window — the port's ruling
        // E17), and the refusal used to leave that unit open, returning `Ok`
        // while the handle rolled back on drop: harmless only because this
        // branch writes nothing, and exactly the shape
        // `tests/unit_of_work_guard.rs` states it cannot see.
        let mut uow = self.ports().database.begin().await?;
        let has_facts = uow
            .commissions()
            .commission_has_facts(&commission.id)
            .await?;
        if !has_facts {
            uow.commissions().delete(&commission.id).await?;
        }
        uow.commit().await?;

        let outcome = if has_facts {
            Outcome::HasFacts
        } else {
            Outcome::Deleted
        };
        Ok(Output { outcome })
    }
}
