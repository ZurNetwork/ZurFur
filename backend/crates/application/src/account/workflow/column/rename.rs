use domain::elements::{
    user::UserId,
    workflow::{ColumnId, ColumnName, WorkflowError, WorkflowId},
};

use crate::{
    account::{
        AccountEntity, AccountError, AccountResult, require_live_account, workflow::column::Columns,
    },
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub column_id: ColumnId,
    pub actor_id: UserId,
    pub workflow_id: WorkflowId,
    pub name: ColumnName,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl Columns<'_> {
    pub async fn rename(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            column_id,
            name,
            workflow_id,
        } = cmd;

        // The board names its own account — one read, no caller-supplied
        // account to reconcile against it.
        let mut workflow = ports
            .workflows
            .find(&workflow_id)
            .await?
            .ok_or(AccountError::NotFound(AccountEntity::Workflow))?;

        require_live_account(ports, &workflow.account_id).await?;

        ports
            .accounts
            .role_of(&actor_id, &workflow.account_id)
            .await?
            // Only an administrative role may reshape a board.
            .filter(|role| role.is_administrative())
            .ok_or(AccountError::IncorrectRole)?;

        let column = workflow
            .rename_column(&column_id, name.clone())
            .map_err(|c| match c {
                WorkflowError::DuplicateColumnName => AccountError::DuplicateName,
                other => AccountError::Infrastructure(anyhow::anyhow!(
                    "the board refused the rename: {other}"
                )),
            })?;

        let mut uow = ports.database.begin().await?;
        uow.columns().rename(column).await?;

        uow.commit().await?;
        Ok(Output)
    }
}
