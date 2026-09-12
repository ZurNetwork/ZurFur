use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::{AccountId, AccountName},
        handle::{Handle, HandleDomain},
        role::Role,
        user::UserId,
    },
};
use shared::settings::{HANDLE_CHANGE_LIMIT, HANDLE_CHANGE_WINDOW, HANDLE_QUARANTINE_WINDOW};

use crate::account::{AccountError, AccountResult, Accounts, require_live_account};

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

        if ports.accounts.find_did_by_handle(&handle).await?.is_some() {
            return Err(AccountError::HandleTaken);
        }

        if ports
            .accounts
            .handle_reserved_for_other(&handle, Some(&account_id), now - HANDLE_QUARANTINE_WINDOW)
            .await?
        {
            return Err(AccountError::HandleTaken);
        };

        ports.did_minter.update_handle(&account.id, &handle).await?;

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
