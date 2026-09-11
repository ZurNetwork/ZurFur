use domain::elements::{
    account::AccountId,
    user::UserId,
    workflow::{Workflow, WorkflowName},
};

use crate::{
    account::{AccountError, AccountResult, workflow::Workflows},
    ports::WithPorts,
};

pub struct Command {
    pub actor_id: UserId,
    pub account_id: AccountId,
    pub workflow_name: WorkflowName,
}
pub struct Output {
    pub workflow: Workflow,
}

impl Workflows<'_> {
    pub async fn create(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            account_id,
            workflow_name,
        } = cmd;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            // Only administrative roles can do
            .filter(|role| role.is_administrative())
            .ok_or(AccountError::IncorrectRole)?;

        let mut uow = self.ports().database.begin().await?;

        let workflow = uow.workflows().create(&workflow_name, &account_id).await?;

        uow.commit().await?;

        Ok(Output { workflow })
    }
}
