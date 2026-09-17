use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, SeatInvitation, SeatInvitationId, element::SeatId},
        invitation::InvitationState,
        user::UserId,
    },
    ports::Unit,
};
use macros::use_case;

use crate::{
    Ports,
    commission::{CommissionError, CommissionResult, invitations::Invitations, require_owner},
    ports::WithPorts,
};

pub struct Command {
    pub actor_id: UserId,
    pub target_id: UserId,
    pub commission_id: CommissionId,
    pub seat_id: SeatId,
}

pub enum Output {
    Created(InvitationOutput),
    PreExisting(InvitationOutput),
}
pub struct InvitationOutput {
    pub invitation_id: SeatInvitationId,
    pub invitation_state: InvitationState,
    pub commission_id: CommissionId,
    pub seat_id: SeatId,
    pub invited_user_id: UserId,
}

impl Invitations<'_> {
    #[use_case]
    pub async fn issue(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            target_id,
            commission_id,
            seat_id,
        } = cmd;
        let commission = require_owner(ports, &commission_id, &actor_id).await?;

        let target_user = uow.users().provision(target_id.did()).await?;
        let seats = ports.commissions.seats(&commission.id).await?;

        // Two distinct answers, deliberately not folded into one: an absent
        // seat is `SeatNotFound`, an occupied one `SeatFilled`.
        let seat = seats
            .iter()
            .find(|seat| seat.id == seat_id)
            .ok_or(CommissionError::SeatNotFound)?;
        if !seat.is_vacant() {
            return Err(CommissionError::SeatFilled);
        }

        if let Some(invitation) = ports
            .commissions
            .find_pending_seat_invitation(&commission.id, &seat.id, &target_user.id)
            .await?
        {
            return Ok(Output::PreExisting(InvitationOutput {
                commission_id: commission.id,
                invitation_id: invitation.id,
                invitation_state: invitation.state,
                invited_user_id: target_user.id,
                seat_id: seat.id,
            }));
        }

        let invitation = SeatInvitation::issue(
            commission.id,
            seat.id,
            target_user.id.clone(),
            target_id.clone(),
            now,
        );
        let minted = invitation.id;

        uow.commissions()
            .create_seat_invitation(&invitation)
            .await?;

        let output = match ports
            .commissions
            .find_pending_seat_invitation(&commission.id, &seat.id, &target_user.id)
            .await?
        {
            Some(stored) => {
                let invitation = InvitationOutput {
                    invitation_id: stored.id,
                    invitation_state: stored.state,
                    commission_id: commission.id,
                    seat_id: seat.id,
                    invited_user_id: target_user.id,
                };
                if stored.id == minted {
                    Output::Created(invitation)
                } else {
                    Output::PreExisting(invitation)
                }
            }
            None => Output::Created(InvitationOutput {
                invitation_id: minted,
                invitation_state: InvitationState::Pending,
                commission_id: commission.id,
                seat_id: seat.id,
                invited_user_id: target_user.id,
            }),
        };
        Ok(output)
    }
}
