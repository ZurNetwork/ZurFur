use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, NewChangelogEntry},
        user::UserId,
    },
};
use serde_json::json;

use crate::{
    commission::{CommissionResult, deadline::Deadline, require_participant},
    ports::WithPorts,
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}

pub struct Output;

impl Deadline<'_> {
    /// Clear the commission's deadline, as a Participant — the honest lever out
    /// of a Late standing (no deadline, nothing to be late against).
    ///
    /// Clearing a deadline that is already absent is an idempotent no-op:
    /// nothing written, nothing appended.
    pub async fn clear(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
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

        let mut uow = ports.database.begin().await?;
        let mut commissions = uow.commissions();
        let moved = commissions.set_deadline(&commission.id, None).await?;
        drop(commissions);
        if moved {
            uow.changelog().append(&entry).await?;
        }
        uow.commit().await?;
        Ok(Output)
    }
}
