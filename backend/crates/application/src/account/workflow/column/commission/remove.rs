use domain::elements::{
    commission::CommissionId,
    user::UserId,
    workflow::{LexOrdering, WorkflowId},
};

use crate::{
    account::{
        AccountEntity, AccountError, AccountResult, require_live_account, workflow::Workflows,
    },
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub workflow_id: WorkflowId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// The column's cards after the removal, in order.
    pub commissions: Vec<CommissionId>,
}

impl Workflows<'_> {
    /// Takes a commission off the board: out of whichever column holds it.
    pub async fn remove(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
            workflow_id,
        } = cmd;

        let account_id = ports
            .workflows
            .owning_account_of(&workflow_id)
            .await?
            .ok_or(AccountError::NotFound(AccountEntity::Workflow))?;
        require_live_account(ports, &account_id).await?;

        // Any member may reposition
        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .ok_or(AccountError::NotAMember)?;

        let mut column = ports
            .columns
            .find_column(&workflow_id, &commission_id)
            .await?
            .ok_or(AccountError::NotFound(AccountEntity::Commission))?;

        column
            .remove_element(commission_id)
            .map_err(|_| AccountError::NotFound(AccountEntity::Commission))?;

        let mut uow = ports.database.begin().await?;
        uow.columns().set_commissions(&column).await?;
        uow.commit().await?;

        // The column has done its job; hand its cards over rather than copy them.
        let commissions = column.into_iter().collect();
        Ok(Output { commissions })
    }
}
