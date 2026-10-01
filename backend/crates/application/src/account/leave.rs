use crate::LazyUnit;
use crate::use_case;
use domain::elements::{account::AccountId, role::Role, user::UserId};

use crate::{
    Ports,
    account::{AccountError, AccountResult, Accounts},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub leaving_user_id: UserId,
    pub account_id: AccountId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl<'a> Accounts<'a> {
    #[use_case]
    pub async fn leave(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
    ) -> AccountResult<Output> {
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

        let uow = uow.open().await?;
        uow.accounts().leave(&leaving_user_id, &account_id).await?;
        Ok(Output)
    }
}
