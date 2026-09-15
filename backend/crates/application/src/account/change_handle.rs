use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::{AccountId, AccountName},
        handle::{Handle, HandleDomain},
        role::Role,
        user::UserId,
    },
};
use shared::settings::{HANDLE_CHANGE_LIMIT, HANDLE_CHANGE_WINDOW};

use crate::account::{
    AccountError, AccountResult, Accounts, ensure_handle_claimable, require_live_account,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub account_id: AccountId,
    pub actor_id: UserId,
    pub handle: Handle,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub id: AccountId,
    pub handle: Handle,
    pub name: AccountName,
}
impl<'a> Accounts<'a> {
    pub async fn change_handle(
        &self,
        cmd: Command,
        handle_domain: &HandleDomain,
        now: DateTimeUtc,
    ) -> AccountResult<Output> {
        let Command {
            account_id,
            actor_id,
            handle,
        } = cmd;
        if !handle.is_in_namespace(handle_domain) {
            return Err(AccountError::UnsupportedHandle);
        };
        let ports = self.ports();
        let account = require_live_account(ports, &account_id).await?;

        if account.handle == handle {
            return Err(AccountError::HandleUnchanged);
        };

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|role| matches!(role, Role::Owner))
            .ok_or(AccountError::IncorrectRole)?;

        let total_changed = ports
            .accounts
            .count_handle_changes_since(&account_id, now - HANDLE_CHANGE_WINDOW)
            .await?;
        if total_changed >= HANDLE_CHANGE_LIMIT {
            return Err(AccountError::RenamedTooRecently);
        };

        ensure_handle_claimable(ports, &handle, handle_domain, Some(&account_id), now).await?;

        ports
            .did_minter
            .update_handle(account.id.did(), &handle)
            .await?;

        let mut uow = ports.database.begin().await?;

        uow.accounts()
            .change_handle(&account_id, &account.handle, &handle, now)
            .await?;

        uow.commit().await?;
        Ok(Output {
            id: account.id,
            handle,
            name: account.name,
        })
    }
}
