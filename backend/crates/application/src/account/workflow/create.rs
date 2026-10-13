use crate::LazyUnit;
use crate::use_case;
use domain::elements::{
    account::AccountId,
    user::UserId,
    workflow::{Workflow, WorkflowName, WorkflowNameError},
};

use crate::{
    Ports,
    account::{AccountError, AccountResult, require_live_account, workflow::Workflows},
    den::{DenName, DenNameError},
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
        #[lazy_unit] uow: &mut LazyUnit<'_>,
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
        let workflow_name = den_workflow_name(&workflow_name)?;

        let uow = uow.open().await?;
        let workflow = uow.workflows().create(&workflow_name, &account_id).await?;

        Ok(Output { workflow })
    }
}

/// A Workflow's name as the Den's name rule keeps it: refused where the rule refuses, and
/// stored without the characters the rule strips.
fn den_workflow_name(workflow_name: &WorkflowName) -> AccountResult<WorkflowName> {
    let name = DenName::try_from(workflow_name.as_ref()).map_err(AccountError::InvalidName)?;
    name.as_ref().parse().map_err(|error| {
        let refusal = match error {
            WorkflowNameError::Empty => DenNameError::Empty,
            WorkflowNameError::TooLong(_) => DenNameError::TooLong,
        };
        AccountError::InvalidName(refusal)
    })
}
