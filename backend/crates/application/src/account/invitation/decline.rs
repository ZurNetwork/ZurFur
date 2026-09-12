use domain::{
    datetime::DateTimeUtc,
    elements::{account::AccountId, user::UserId},
};

use crate::{
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
    pub async fn decline(&self, cmd: Command, now: DateTimeUtc) -> AccountResult<Output> {
        let ports = self.ports();
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
        let mut uow = self.ports().database.begin().await?;
        uow.accounts().revoke_invitation(&invitation.id).await?;
        uow.commit().await?;
        Ok(Output)
    }
}
