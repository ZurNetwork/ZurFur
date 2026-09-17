use domain::{
    elements::{account::AccountId, role::Role, user::UserId},
    ports::Unit,
};
use macros::use_case;

use crate::{
    Ports,
    account::{AccountError, AccountResult, Accounts, require_live_account},
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub account_id: AccountId,
    pub actor_id: UserId,
    pub target_id: UserId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub account_id: AccountId,
    pub owner_id: UserId,
    pub previous_owner_id: UserId,
}

impl Accounts<'_> {
    #[use_case]
    pub async fn transfer_ownership(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
    ) -> AccountResult<Output> {
        let Command {
            account_id,
            actor_id,
            target_id,
        } = cmd;

        if actor_id == target_id {
            return Err(AccountError::CannotTransferToSelf);
        }

        require_live_account(ports, &account_id).await?;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|role| matches!(role, Role::Owner))
            .ok_or(AccountError::IncorrectRole)?;

        ports
            .accounts
            .role_of(&target_id, &account_id)
            .await?
            .ok_or(AccountError::NotAMember)?;

        uow.accounts()
            .transfer_ownership(&actor_id, &target_id, &account_id)
            .await?;
        Ok(Output {
            account_id,
            owner_id: target_id,
            previous_owner_id: actor_id,
        })
    }
}
