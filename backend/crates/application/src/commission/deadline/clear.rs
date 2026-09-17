use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, NewChangelogEntry},
        user::UserId,
    },
    ports::Unit,
};
use macros::use_case;
use serde_json::json;

use crate::{
    Ports,
    commission::{CommissionResult, deadline::Deadline, require_participant},
    ports::WithPorts,
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}

pub struct Output;

impl Deadline<'_> {
    /// Clear the commission's deadline, as a Participant — the lever out of a
    /// Late standing. Clearing an already-absent deadline is a no-op.
    #[use_case]
    pub async fn clear(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        let commission = require_participant(ports, &commission_id, &actor_id).await?;

        if commission.deadline.is_none() {
            return Ok(Output);
        }

        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::DeadlineSet,
            actor_id,
            json!({ "from": commission.deadline, "to": None as Option<DateTimeUtc> }),
            now,
        );

        let mut commissions = uow.commissions();
        let moved = commissions.set_deadline(&commission.id, None).await?;
        drop(commissions);
        if moved {
            uow.changelog().append(&entry).await?;
        }
        Ok(Output)
    }
}
