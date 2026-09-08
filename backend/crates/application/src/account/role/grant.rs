use domain::elements::{account::AccountId, role::Role, user::UserId, user_account::UserAccount};

use crate::{
    account::{AccountError, AccountResult, require_live_account, role::Roles},
    ports::WithPorts,
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
    pub async fn grant(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            account_id,
            target_id,
            actor_id,
            role,
        } = cmd;
        // Owner is the one role a grant may never mint: handing over ownership
        // is a *transfer*, with its own use case and its own rules ("an Owner
        // never has a parent, even when transferred"). The guard refused every
        // role *except* Owner, which — since `can_grant` also never permits
        // Owner — meant no grant of any kind could succeed.
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

        if let Some(current_role) = ports.accounts.role_of(&target_id, &account_id).await?
            && !actor_role.can_grant(&current_role)
        {
            return Err(AccountError::IncorrectRole);
        }

        // Provisioning is a write, so it happens only once the actor's standing
        // is settled: an unauthorized grant must not leave a User row behind for
        // the DID it named.
        let mut uow = self.ports().database.begin().await?;
        let target = uow.users().provision(&target_id).await?;

        let member = UserAccount {
            user_id: target.id.clone(),
            account_id: account_id.clone(),
            alias: None,
            role: role.clone(),
        };

        uow.accounts().grant_role(&member).await?;

        uow.commit().await?;
        Ok(Output {
            account_id,
            role,
            user_id: target.id.clone(),
        })
    }
}
