use crate::LazyUnit;
use crate::use_case;
use domain::elements::{account::AccountId, role::Role, user::UserId};

use crate::{
    Ports,
    account::{AccountError, AccountResult, require_live_account, role::Roles},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub account_id: AccountId,
    pub target_id: UserId,
    pub actor_id: UserId,
    pub role: Role,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub account_id: AccountId,
    pub role: Role,
    pub user_id: UserId,
}
impl Roles<'_> {
    #[use_case]
    pub async fn grant(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
    ) -> AccountResult<Output> {
        let Command {
            account_id,
            target_id,
            actor_id,
            role,
        } = cmd;
        // Owner is the one role a grant may never mint: handing over ownership
        // is a transfer, with its own use case.
        if matches!(role, Role::Owner) {
            return Err(AccountError::IncorrectTransferOfAccount);
        };

        require_live_account(ports, &account_id).await?;

        let actor_role = ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|r| r.can_grant(&role))
            .ok_or(AccountError::IncorrectRole)?;

        let target_role = ports
            .accounts
            .role_of(&target_id, &account_id)
            .await?
            .ok_or(AccountError::NotAMember)?;

        if !actor_role.can_grant(&target_role) {
            return Err(AccountError::IncorrectRole);
        }

        let uow = uow.open().await?;
        uow.accounts()
            .grant_role(&target_id, &account_id, &role)
            .await?;
        Ok(Output {
            account_id,
            role,
            user_id: target_id,
        })
    }
}
