use domain::{
    elements::{
        account::AccountId,
        user::UserId,
        workflow::{Workflow, WorkflowName},
    },
    ports::Unit,
};
use macros::use_case;

use crate::{
    Ports,
    account::{AccountError, AccountResult, require_live_account, workflow::Workflows},
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
    #[use_case]
    pub async fn create(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
    ) -> AccountResult<Output> {
        let Command {
            actor_id,
            account_id,
            workflow_name,
        } = cmd;

        require_live_account(ports, &account_id).await?;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            // Only administrative roles can do
            .filter(|role| role.is_administrative())
            .ok_or(AccountError::IncorrectRole)?;

        let workflow = uow.workflows().create(&workflow_name, &account_id).await?;

        Ok(Output { workflow })
    }
}
