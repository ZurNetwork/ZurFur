use crate::LazyUnit;
use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            CommissionId, CommissionState, DeadlineChange, NewChangelogEntry, clear_deadline,
        },
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{CommissionResult, deadline::Deadline, require_participant},
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}

pub struct Output;

impl Deadline<'_> {
    /// Clear the commission's deadline, as a Participant — the lever out of a
    /// Late standing. Clearing an already-absent deadline is a no-op, and a Disputed
    /// commission's is refused.
    #[use_case]
    pub async fn clear(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        let commission = require_participant(ports, &commission_id, &actor_id).await?;

        let state = CommissionState::from(&commission);
        let Some(cleared) = clear_deadline(&state)? else {
            return Ok(Output);
        };
        let entry = NewChangelogEntry::deadline_cleared(commission.id, actor_id, &cleared, now);
        let change = DeadlineChange::from(cleared);

        let uow = uow.open().await?;
        let mut commissions = uow.commissions();
        let moved = commissions
            .set_deadline(&commission.id, change.after())
            .await?;
        drop(commissions);
        if moved {
            uow.changelog().append(&entry).await?;
        }
        Ok(Output)
    }
}
