use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, element::SeatId},
        user::UserId,
    },
};

use crate::{
    commission::{CommissionError, CommissionResult, invitations::Invitations, require_owner},
    ports::WithPorts,
};

pub struct Command {
    pub actor_id: UserId,
    pub target_id: UserId,
    pub commission_id: CommissionId,
    pub seat_id: SeatId,
}
pub struct Output {
    pub commission_id: CommissionId,
    pub seat_id: SeatId,
    pub user_id: UserId,
}

impl Invitations<'_> {
    pub async fn revoke(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            target_id,
            commission_id,
            seat_id,
        } = cmd;

        require_owner(ports, &commission_id, &actor_id).await?;

        // No seat-existence gate on purpose: the lookup is already
        // commission-scoped, so a foreign seat id is a bare no-op, never a
        // `404` confirming the id is real somewhere.
        let Some(mut invitation) = ports
            .commissions
            .find_pending_seat_invitation(&commission_id, &seat_id, &target_id)
            .await?
        else {
            return Ok(Output {
                commission_id,
                seat_id,
                user_id: target_id,
            });
        };

        invitation
            .revoke(now)
            .map_err(|_| CommissionError::InvalidStateRequested)?;
        let mut uow = self.ports().database.begin().await?;
        uow.commissions()
            .revoke_seat_invitation(&invitation.id)
            .await?;
        uow.commit().await?;

        Ok(Output {
            commission_id,
            seat_id,
            user_id: target_id,
        })
    }
}
