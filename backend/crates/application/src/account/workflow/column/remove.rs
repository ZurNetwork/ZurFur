use domain::{
    elements::{role::Role, user::UserId, workflow::ColumnId},
    ports::Unit,
};
use macros::use_case;

use crate::{
    Ports,
    account::{
        AccountEntity, AccountError, AccountResult, require_live_account, workflow::column::Columns,
    },
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub column_id: ColumnId,
    pub actor_id: UserId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl Columns<'_> {
    #[use_case]
    pub async fn remove(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
    ) -> AccountResult<Output> {
        let Command {
            actor_id,
            column_id,
        } = cmd;

        let account_id = ports
            .columns
            .owning_account_of(&column_id)
            .await?
            .ok_or(AccountError::NotFound(AccountEntity::Column))?;
        require_live_account(ports, &account_id).await?;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|role| matches!(role, Role::Owner | Role::Admin))
            .ok_or(AccountError::IncorrectRole)?;

        if ports.columns.has_commissions(&column_id).await? {
            return Err(AccountError::ContainsCommissions);
        }

        uow.columns().delete(&column_id).await?;
        Ok(Output)
    }
}
