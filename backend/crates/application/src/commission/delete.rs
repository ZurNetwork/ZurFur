use crate::LazyUnit;
use crate::use_case;
use domain::elements::{
    commission::{CommissionId, CommissionRole},
    user::UserId,
};

use crate::{
    Ports,
    commission::{CommissionResult, Commissions, require_or_not_found},
    common_error::{CommonError, NotFoundEntity},
};

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
    /// Hard-deletes the commission, but only in Draft and fact-free, decided on its row read
    /// under lock in the deleting unit. Owner-only; every other caller gets the closed door's
    /// `CommissionNotFound`. A commission past Draft or bearing a fact is untouched and
    /// reported as [`Outcome::HasFacts`].
    #[use_case]
    pub async fn delete(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        require_or_not_found(ports, &commission_id, &actor_id, CommissionRole::Owner).await?;

        // One unit, closed on every branch: the step check, the fact gate and the delete
        // they guard share a transaction over the locked row, or a move between the pool
        // read and the delete would slip through.
        let uow = uow.open().await?;
        let locked = uow
            .commissions()
            .find_for_update(&commission_id)
            .await?
            .filter(|c| c.is_owned_by(&actor_id))
            .ok_or(CommonError::NotFound(NotFoundEntity::Commission))?;
        if !locked.lifecycle_step.allows_hard_delete() {
            return Ok(Output {
                outcome: Outcome::HasFacts,
            });
        }
        let has_facts = uow
            .commissions()
            .commission_has_facts(&commission_id)
            .await?;
        if !has_facts {
            #[expect(
                clippy::disallowed_methods,
                reason = "the ungated delete site, frozen until its slice routes it through the den's write gate"
            )]
            uow.commissions().delete(&commission_id).await?;
        }

        let outcome = if has_facts {
            Outcome::HasFacts
        } else {
            Outcome::Deleted
        };
        Ok(Output { outcome })
    }
}
