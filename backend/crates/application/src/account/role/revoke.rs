use domain::elements::{account::AccountId, user::UserId};

use crate::{
    account::{AccountError, AccountResult, require_live_account, role::Roles},
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub account_id: AccountId,
    pub target_id: UserId,
    pub actor_id: UserId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub account_id: AccountId,
    pub user_id: UserId,
}

impl Roles<'_> {
    pub async fn revoke(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            account_id,
            target_id,
            actor_id,
        } = cmd;

        require_live_account(ports, &account_id).await?;

        // The actor's own standing is settled BEFORE the target's membership is
        // looked at, as `grant` already does. The order is load-bearing: the two
        // refusals below are told apart on the wire (`404 member_not_found` vs
        // `403 forbidden`), so probing the target first would answer "is X a
        // member of this account?" for any signed-in caller, member or not.
        let actor_role = ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .ok_or(AccountError::IncorrectRole)?;

        // A target who holds no role here is a *missing membership*, not a
        // refusal — every sibling use case (`leave`, `transfer_ownership`,
        // `invitation::revoke`) already answers `NotAMember`. This one answered
        // `UserNotFound`, which the driver renders `403`: a revoke of someone
        // who was never a member reported as "you may not", not "there is no
        // such member".
        let target_role = ports
            .accounts
            .role_of(&target_id, &account_id)
            .await?
            .ok_or(AccountError::NotAMember)?;

        if !actor_role.can_grant(&target_role) {
            return Err(AccountError::IncorrectRole);
        }

        let mut uow = ports.database.begin().await?;
        uow.accounts().revoke_role(&target_id, &account_id).await?;

        uow.commit().await?;
        Ok(Output {
            account_id,
            user_id: target_id,
        })
    }
}
