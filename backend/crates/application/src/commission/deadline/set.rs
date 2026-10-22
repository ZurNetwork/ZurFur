use crate::LazyUnit;
use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            CommissionId, CommissionState, DeadlineChange, NewChangelogEntry, schedule_deadline,
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
    pub deadline: DateTimeUtc,
}
pub struct Output;

impl Deadline<'_> {
    /// Set — or move — the commission's deadline, as a Participant. Re-setting
    /// the deadline already held is a no-op, and a Disputed commission's is refused. A
    /// later deadline than the one held records as `DeadlineExtended`, anything else as
    /// `DeadlineSet`.
    #[use_case]
    pub async fn set(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
            deadline,
        } = cmd;
        let commission = require_participant(ports, &commission_id, &actor_id).await?;

        // The no-op is "the stored deadline already IS the requested one" —
        // never a comparison against `now`.
        let state = CommissionState::from(&commission);
        let Some(set) = schedule_deadline(&state, deadline)? else {
            return Ok(Output);
        };
        let entry = NewChangelogEntry::deadline_set(commission.id, actor_id, &set, now);
        let change = DeadlineChange::from(set);

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
