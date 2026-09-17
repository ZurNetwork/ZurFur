use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::{AccountId, AccountName},
        handle::{Handle, HandleDomain},
        role::Role,
        user::UserId,
    },
    ports::Unit,
};
use macros::use_case;
use shared::settings::{HANDLE_CHANGE_LIMIT, HANDLE_CHANGE_WINDOW};

use crate::{
    Ports,
    account::{
        AccountError, AccountResult, Accounts, ensure_handle_claimable, require_live_account,
    },
    ports::WithPorts,
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
    #[use_case]
    pub async fn change_handle(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
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

        uow.accounts()
            .change_handle(&account_id, &account.handle, &handle, now)
            .await?;

        Ok(Output {
            id: account.id,
            handle,
            name: account.name,
        })
    }
}
