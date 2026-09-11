use domain::elements::{account::AccountId, role::Role, user::UserId};

use crate::account::{AccountError, AccountResult, Accounts};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub leaving_user_id: UserId,
    pub account_id: AccountId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl<'a> Accounts<'a> {
    pub async fn leave(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            account_id,
            leaving_user_id,
        } = cmd;

        match ports
            .accounts
            .role_of(&leaving_user_id, &account_id)
            .await?
        {
            Some(Role::Owner) => return Err(AccountError::OwnerCannotLeave),
            Some(_) => {}
            None => return Err(AccountError::NotAMember),
        };

        let mut uow = ports.database.begin().await?;
        uow.accounts().leave(&leaving_user_id, &account_id).await?;
        uow.commit().await?;
        Ok(Output)
    }
}
