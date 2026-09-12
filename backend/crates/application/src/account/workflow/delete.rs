use domain::elements::{user::UserId, workflow::WorkflowId};

use crate::{
    account::{
        AccountEntity, AccountError, AccountResult, require_live_account, workflow::Workflows,
    },
    ports::WithPorts,
};

pub struct Command {
    pub actor_id: UserId,
    pub workflow_id: WorkflowId,
}
pub struct Output;

impl Workflows<'_> {
    pub async fn delete(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            workflow_id,
        } = cmd;

        let account_id = ports
            .workflows
            .owning_account_of(&workflow_id)
            .await?
            .ok_or(AccountError::NotFound(AccountEntity::Workflow))?;

        require_live_account(ports, &account_id).await?;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            // Only an administrative role may delete a board.
            .filter(|role| role.is_administrative())
            .ok_or(AccountError::IncorrectRole)?;

        let mut uow = self.ports().database.begin().await?;

        uow.workflows().delete(&workflow_id).await?;

        uow.commit().await?;
        Ok(Output)
    }
}
