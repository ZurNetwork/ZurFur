use crate::ports::WithPorts;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            CommissionId, CommissionState, DeadlineStatus, DelayedChange, NewChangelogEntry,
            clear_delayed, set_delayed,
        },
        user::UserId,
    },
};

use crate::LazyUnit;
use crate::commission::{CommissionResult, deadline::Deadline, require_participant};

pub mod clear;
pub mod set;

/// Raise the Delayed flag (`to` is `Some`) or take it down (`None`) — the body
/// [`set`](set) and [`clear`](clear) share. Participant-gated; a `Late` derived at
/// `now` refuses both directions; the flag may only be raised on a commission
/// that has a deadline. Keyed on a real transition, so a no-op records nothing.
async fn apply(
    ports: &crate::Ports,
    uow: &mut LazyUnit<'_>,
    commission_id: &CommissionId,
    actor_id: UserId,
    to: Option<DeadlineStatus>,
    now: DateTimeUtc,
) -> CommissionResult<()> {
    let commission = require_participant(ports, commission_id, &actor_id).await?;

    let state = CommissionState::from(&commission);
    let (entry, change) = match to {
        Some(_) => {
            let Some(set) = set_delayed(&state, now)? else {
                return Ok(());
            };
            let entry = NewChangelogEntry::delayed_set(commission.id, actor_id, &set, now);
            (entry, DelayedChange::from(set))
        }
        None => {
            let Some(cleared) = clear_delayed(&state, now)? else {
                return Ok(());
            };
            let entry = NewChangelogEntry::delayed_cleared(commission.id, actor_id, &cleared, now);
            (entry, DelayedChange::from(cleared))
        }
    };

    let uow = uow.open().await?;
    let mut commissions = uow.commissions();
    let moved = commissions
        .set_deadline_status(&commission.id, change.after())
        .await?;
    drop(commissions);
    if moved {
        uow.changelog().append(&entry).await?;
    }
    Ok(())
}

#[derive(WithPorts)]
pub struct Status<'a> {
    #[ports]
    pub deadline: &'a Deadline<'a>,
}
