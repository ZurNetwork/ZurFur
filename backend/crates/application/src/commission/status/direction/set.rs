use crate::LazyUnit;
use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            CommissionId, CommissionState, DirectionChange, DirectionStatus, NewChangelogEntry,
            clear_direction, set_direction,
        },
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{CommissionResult, require_participant, status::direction::Direction},
};

pub struct Command {
    pub user_id: UserId,
    pub commission_id: CommissionId,
    pub direction: Option<DirectionStatus>,
}
pub struct Output;

impl Direction<'_> {
    #[use_case]
    pub async fn set(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            user_id,
            commission_id,
            direction,
        } = cmd;
        // Authorize first, then notice the no-op: answering early would hand an
        // outsider a different reply than the closed door — an existence oracle.
        let commission = require_participant(ports, &commission_id, &user_id).await?;

        let state = CommissionState::from(&commission);
        let (entry, change) = match direction {
            Some(to) => {
                let Some(set) = set_direction(&state, to) else {
                    return Ok(Output);
                };
                let entry = NewChangelogEntry::direction_set(commission.id, user_id, &set, now);
                (entry, DirectionChange::from(set))
            }
            None => {
                let Some(cleared) = clear_direction(&state) else {
                    return Ok(Output);
                };
                let entry =
                    NewChangelogEntry::direction_cleared(commission.id, user_id, &cleared, now);
                (entry, DirectionChange::from(cleared))
            }
        };

        let uow = uow.open().await?;
        let mut commissions = uow.commissions();
        let moved = commissions
            .set_direction_status(&commission.id, change.after())
            .await?;
        drop(commissions);
        if moved {
            uow.changelog().append(&entry).await?;
        }
        Ok(Output)
    }
}
