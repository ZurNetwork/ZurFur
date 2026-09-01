use domain::elements::{
    account::AccountId,
    role::Role,
    user::UserId,
    workflow::{ColumnId, ColumnName, WorkflowError, WorkflowId},
};

use crate::{
    account::{AccountEntity, AccountError, AccountResult, workflow::column::Columns},
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub column_id: ColumnId,
    pub actor_id: UserId,
    pub workflow_id: WorkflowId,
    pub account_id: AccountId,
    pub name: ColumnName,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl Columns<'_> {
    pub async fn rename(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            account_id,
            column_id,
            name,
            workflow_id,
        } = cmd;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|role| matches!(role, Role::Owner | Role::Admin))
            .ok_or(AccountError::IncorrectRole)?;

        let mut workflow = ports
            .workflows
            .find(&workflow_id)
            .await?
            .ok_or(AccountError::NotFound(AccountEntity::Workflow))?;

        let column = workflow
            .rename_column(&column_id, name.clone())
            .map_err(|c| match c {
                WorkflowError::DuplicateColumnName => AccountError::DuplicateName,
                _ => AccountError::Infrastructure(anyhow::anyhow!("Unknown error")),
            })?;

        let mut uow = ports.database.begin().await?;
        uow.columns().rename(column).await?;

        uow.commit().await?;
        Ok(Output)
    }
}
