use domain::elements::{account::AccountId, role::Role, user::UserId};

use crate::{
    account::{AccountError, AccountResult, invitation::Invitations, require_live_account},
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub target_id: UserId,
    pub account_id: AccountId,
    pub listed_on_profile: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub account_id: AccountId,
    pub role: Role,
    pub user_id: UserId,
}

impl Invitations<'_> {
    /// Accepts the caller's own pending invitation, seating them at the offered
    /// role. Answers `AccountNotFound` for a dead account and
    /// `NoPendingInvitation` when no live offer names them.
    pub async fn accept(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            target_id,
            account_id,
            listed_on_profile,
        } = cmd;
        require_live_account(ports, &account_id).await?;
        let invitation = ports
            .accounts
            .find_pending_invitation(&account_id, &target_id)
            .await?
            .ok_or(AccountError::NoPendingInvitation)?;

        let mut uow = ports.database.begin().await?;

        let result = uow
            .accounts()
            .accept_invitation(invitation, listed_on_profile)
            .await?;

        uow.commit().await?;
        Ok(Output {
            account_id: result.account_id,
            role: result.role,
            user_id: result.user_id,
        })
    }
}
