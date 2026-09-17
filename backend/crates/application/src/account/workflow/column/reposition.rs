use domain::{
    elements::{
        user::UserId,
        workflow::{ColumnId, LexOrdering, WorkflowId},
    },
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
    pub actor_id: UserId,
    pub column_id: ColumnId,
    pub workflow_id: WorkflowId,
    pub to_index: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl Columns<'_> {
    #[use_case]
    pub async fn reposition(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
    ) -> AccountResult<Output> {
        let Command {
            actor_id,
            column_id,
            workflow_id,
            to_index,
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

        let from_index = workflow
            .iter()
            .position(|c| c.id == column_id)
            .ok_or(AccountError::NotFound(AccountEntity::Column))?;
        if from_index == to_index {
            return Err(AccountError::NothingToDo);
        }
        workflow.relocate(from_index, to_index)?;

        uow.workflows().set_indexes(&workflow).await?;
        Ok(Output)
    }
}
