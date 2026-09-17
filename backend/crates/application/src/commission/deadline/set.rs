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
    pub deadline: DateTimeUtc,
}
pub struct Output;

impl Deadline<'_> {
    /// Set — or move — the commission's deadline, as a Participant. Re-setting
    /// the deadline already held is a no-op. A later deadline than the one held
    /// records as `DeadlineExtended`, anything else as `DeadlineSet`.
    #[use_case]
    pub async fn set(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
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

        let mut commissions = uow.commissions();
        let moved = commissions
            .set_deadline(&commission.id, Some(deadline))
            .await?;
        drop(commissions);
        if moved {
            uow.changelog().append(&entry).await?;
        }
        Ok(Output)
    }
}
