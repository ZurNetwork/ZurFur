use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, DeadlineStatus, NewChangelogEntry},
        user::UserId,
    },
};
use serde_json::json;

use crate::{
    commission::{
        CommissionError, CommissionResult, deadline::Deadline, deadline::DeadlineSetEventPayload,
        require_participant,
    },
    ports::WithPorts,
};

pub mod clear;
pub mod set;

/// Move the deadline-axis status to `to` (`None` clears it) — the body
/// [`set`](set) and [`clear`](clear) share. Participant-gated; a standing
/// `Late` refuses both directions; a status may only be set on a commission
/// that has a deadline. Keyed on a real transition, so a no-op records nothing.
async fn apply(
    ports: &crate::Ports,
    commission_id: &CommissionId,
    actor_id: UserId,
    to: Option<DeadlineStatus>,
    now: DateTimeUtc,
) -> CommissionResult<()> {
    let commission = require_participant(ports, commission_id, &actor_id).await?;

    if commission.deadline_status == Some(DeadlineStatus::Late) {
        return Err(CommissionError::CommissionLate);
    }
    if to.is_some() && commission.deadline.is_none() {
        return Err(CommissionError::NoDeadline);
    }

    let payload = DeadlineSetEventPayload {
        from: commission.deadline_status.map(|s| s.to_string()),
        to: to.map(|s| s.to_string()),
        deadline: commission.deadline,
    };
    let entry = NewChangelogEntry::event(
        commission.id,
        ChangelogEntryKind::Delayed,
        actor_id,
        json!({
            "from": payload.from,
            "to": payload.to,
            "deadline": payload.deadline,
        }),
        now,
    );

    let mut uow = ports.database.begin().await?;
    let mut commissions = uow.commissions();
    let moved = commissions.set_deadline_status(&commission.id, to).await?;
    drop(commissions);
    if moved {
        uow.changelog().append(&entry).await?;
    }
    uow.commit().await?;
    Ok(())
}

pub struct Status<'a> {
    pub deadline: &'a Deadline<'a>,
}

impl<'a> WithPorts<'a> for Status<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.deadline.ports()
    }
}

impl<'a> Deadline<'a> {
    pub fn status(&'a self) -> Status<'a> {
        Status { deadline: self }
    }
}
