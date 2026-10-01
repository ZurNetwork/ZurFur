use crate::LazyUnit;
use crate::use_case;
use domain::elements::{user::UserId, workflow::WorkflowId};

use crate::{
    Ports,
    account::{
        AccountEntity, AccountError, AccountResult, require_live_account, workflow::Workflows,
    },
};

pub struct Command {
    pub actor_id: UserId,
    pub workflow_id: WorkflowId,
}
pub struct Output;

impl Workflows<'_> {
    #[use_case]
    pub async fn delete(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
    ) -> AccountResult<Output> {
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

        let uow = uow.open().await?;
        uow.workflows().delete(&workflow_id).await?;

        Ok(Output)
    }
}
