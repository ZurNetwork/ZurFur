use domain::elements::{role::Role, user::UserId, workflow::ColumnId};

use crate::{
    account::{AccountError, AccountResult, workflow::column::Columns},
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
    pub async fn remove(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            column_id,
        } = cmd;

        let account_id = ports.columns.owning_account_of(&column_id).await?;
        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|role| matches!(role, Role::Owner | Role::Admin))
            .ok_or(AccountError::IncorrectRole)?;

        if ports.columns.has_commissions(&column_id).await? {
            return Err(AccountError::ContainsCommissions);
        }

        let mut uow = ports.database.begin().await?;
        uow.columns().delete(&column_id).await?;
        uow.commit().await?;
        todo!()
    }
}
