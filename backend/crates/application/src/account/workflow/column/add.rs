use domain::elements::{
    role::Role,
    user::UserId,
    workflow::{
        Column, ColumnName, LexOrdering, MAX_COLUMNS_PER_WORKFLOW, WorkflowError, WorkflowId,
    },
};

use crate::{
    account::{
        AccountEntity, AccountError, AccountResult, require_live_account, workflow::column::Columns,
    },
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub actor_id: UserId,
    pub workflow_id: WorkflowId,
    pub column_name: ColumnName,
    pub position: u8,
}
#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub columns: Vec<Column>,
}

impl Columns<'_> {
    pub async fn add(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            workflow_id,
            column_name,
            position,
        } = cmd;

        // The board is fetched first because it is how the owning account is
        // learned — but nothing about its CONTENTS may be inspected before the
        // actor's standing is settled, or a non-member could tell an existing
        // board (and a name already on it) from an absent one.
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
            .filter(|role| matches!(role, Role::Owner | Role::Admin))
            .ok_or(AccountError::IncorrectRole)?;

        if workflow.iter().any(|column| column.name == column_name) {
            return Err(AccountError::DuplicateName);
        }

        if workflow.len() + 1 > MAX_COLUMNS_PER_WORKFLOW {
            return Err(AccountError::IncorrectNumberOfColumns);
        }

        let new_column = workflow.new_column(column_name, workflow.visibility.clone());
        workflow
            .insert(usize::from(position), new_column)
            .map_err(|e| match e {
                WorkflowError::DuplicateColumnName => AccountError::DuplicateName,
                other => AccountError::Infrastructure(anyhow::anyhow!(
                    "the board refused the column: {other}"
                )),
            })?;

        let mut uow = ports.database.begin().await?;
        uow.workflows().set_indexes(&workflow).await?;
        uow.commit().await?;

        let columns = workflow.iter().cloned().collect();
        Ok(Output { columns })
    }
}
