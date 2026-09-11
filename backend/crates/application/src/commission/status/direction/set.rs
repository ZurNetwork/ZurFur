use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, DirectionStatus, NewChangelogEntry},
        user::UserId,
    },
};
use serde_json::json;

use crate::{
    commission::{CommissionResult, require_participant, status::direction::Direction},
    ports::WithPorts,
};

pub struct Command {
    pub user_id: UserId,
    pub commission_id: CommissionId,
    pub direction: Option<DirectionStatus>,
}
pub struct Output;

impl Direction<'_> {
    pub async fn set(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            user_id,
            commission_id,
            direction,
        } = cmd;
        // Authorize first, *then* notice the no-op: answering "already at that
        // value" before the membership check would hand an outsider a different
        // reply than the closed door gives, which is an existence oracle.
        let commission = require_participant(ports, &commission_id, &user_id).await?;

        if commission.direction_status == direction {
            return Ok(Output);
        }

        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::StatusChanged,
            user_id,
            json!({
                "from": commission.direction_status.map(|s| s.to_string()),
                "to": direction.map(|s| s.to_string())
            }),
            now,
        );

        let mut uow = ports.database.begin().await?;
        let mut commissions = uow.commissions();
        let moved = commissions
            .set_direction_status(&commission.id, direction)
            .await?;
        drop(commissions);
        if moved {
            uow.changelog().append(&entry).await?;
        }
        uow.commit().await?;
        Ok(Output)
    }
}
