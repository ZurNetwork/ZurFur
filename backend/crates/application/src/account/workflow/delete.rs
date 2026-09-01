use domain::elements::{role::Role, user::UserId, workflow::WorkflowId};

use crate::{
    account::{AccountError, AccountResult, workflow::Workflows},
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

        let account_id = ports.workflows.owning_account_of(&workflow_id).await?;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            // Only administrative roles can do
            .filter(|role| !matches!(role, Role::Owner | Role::Admin))
            .ok_or(AccountError::IncorrectRole)?;

        let mut uow = self.ports().database.begin().await?;

        uow.workflows().delete(&workflow_id).await?;

        uow.commit().await?;
        Ok(Output)
    }
}
