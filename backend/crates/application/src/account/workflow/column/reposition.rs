use domain::elements::{
    account::AccountId,
    user::UserId,
    workflow::{ColumnId, LexOrdering, WorkflowId},
};

use crate::{
    account::{AccountEntity, AccountError, AccountResult, workflow::column::Columns},
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub actor_id: UserId,
    pub column_id: ColumnId,
    pub workflow_id: WorkflowId,
    pub account_id: AccountId,
    pub to_index: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl Columns<'_> {
    pub async fn reposition(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            account_id,
            actor_id,
            column_id,
            workflow_id,
            to_index,
        } = cmd;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|role| role.is_administrative())
            .ok_or(AccountError::IncorrectRole)?;

        let mut workflow = ports
            .workflows
            .find(&workflow_id)
            .await?
            .ok_or(AccountError::NotFound(AccountEntity::Workflow))?;

        let from_index = workflow
            .iter()
            .position(|c| c.id == column_id)
            .ok_or(AccountError::NotFound(AccountEntity::Column))?;
        if from_index == to_index {
            return Err(AccountError::NothingToDo);
        }
        workflow.relocate(from_index, to_index)?;

        let mut uow = ports.database.begin().await?;
        uow.workflows().set_indexes(&workflow).await?;
        uow.commit().await?;
        todo!()
    }
}
