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
    pub deadline: DateTimeUtc,
}
pub struct Output;

impl Deadline<'_> {
    /// Set — or move — the commission's deadline, as a Participant.
    ///
    /// Re-setting the deadline already held is an idempotent no-op: nothing is
    /// written and nothing is appended. A later deadline than the one held
    /// records as `DeadlineExtended`, anything else as `DeadlineSet`.
    pub async fn set(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
            deadline,
        } = cmd;
        let commission = require_participant(ports, &commission_id, &actor_id).await?;

        // The no-op is "the stored deadline already IS the requested one".
        // Comparing the stored deadline to `now` instead — as this did — is
        // never true in practice, so every repeat set appended a fresh entry.
        if commission.deadline == Some(deadline) {
            return Ok(Output);
        }

        let kind = match commission.deadline {
            Some(old) if deadline > old => ChangelogEntryKind::DeadlineExtended,
            _ => ChangelogEntryKind::DeadlineSet,
        };

        let entry = NewChangelogEntry::event(
            commission.id,
            kind,
            actor_id,
            json!({ "from": commission.deadline, "to": deadline }),
            now,
        );

        let mut uow = ports.database.begin().await?;
        let mut commissions = uow.commissions();
        let moved = commissions
            .set_deadline(&commission.id, Some(deadline))
            .await?;
        drop(commissions);
        if moved {
            uow.changelog().append(&entry).await?;
        }
        uow.commit().await?;
        Ok(Output)
    }
}
