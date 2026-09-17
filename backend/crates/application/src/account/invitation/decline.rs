use domain::{
    datetime::DateTimeUtc,
    elements::{account::AccountId, user::UserId},
    ports::Unit,
};
use macros::use_case;

use crate::{
    Ports,
    account::{AccountError, AccountResult, invitation::Invitations},
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub account_id: AccountId,
    pub actor_id: UserId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl Invitations<'_> {
    #[use_case]
    pub async fn decline(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> AccountResult<Output> {
        let Command {
            account_id,
            actor_id,
        } = cmd;

        let mut invitation = ports
            .accounts
            .find_pending_invitation(&account_id, &actor_id)
            .await?
            .ok_or(AccountError::NoPendingInvitation)?;

        invitation.revoke(now).map_err(|_| {
            AccountError::Infrastructure(anyhow::anyhow!(
                "The invitation could not be declined, as it is no longer pending"
            ))
        })?;
        uow.accounts().revoke_invitation(&invitation.id).await?;
        Ok(Output)
    }
}
