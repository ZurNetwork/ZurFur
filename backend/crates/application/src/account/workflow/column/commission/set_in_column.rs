use domain::elements::{
    commission::{CommissionId, Visibility},
    user::UserId,
    workflow::{ColumnId, LexOrdering, WorkflowError},
};

use crate::{
    Ports,
    account::{AccountEntity, AccountError, AccountResult},
    commission::Commissions,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub commission_id: CommissionId,
    pub column_id: ColumnId,
    pub index: usize,
    pub actor_id: UserId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub commissions: Vec<CommissionId>,
}

async fn has_own_view(
    ports: &Ports,
    actor_id: &UserId,
    commission_id: &CommissionId,
) -> Result<bool, AccountError> {
    Ok(ports
        .commissions
        .is_participant(commission_id, actor_id)
        .await?
        || ports
            .commissions
            .view_grant(commission_id, actor_id)
            .await?
            .is_some())
}

impl Commissions<'_> {
    pub async fn insert_in_column(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            column_id,
            commission_id,
            index,
        } = cmd;

        // Board authority is settled before the commission is looked up: a
        // non-member must learn nothing about an id they invented.
        let mut column = ports
            .columns
            .find(&column_id)
            .await?
            .ok_or(AccountError::NotFound(AccountEntity::Column))?;

        let account_id = ports
            .workflows
            .owning_account_of(&column.workflow_id)
            .await?;

        // Any member may put a card on their account's board.
        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .ok_or(AccountError::NotAMember)?;

        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .ok_or(AccountError::NotFound(AccountEntity::Commission))?;

        // Two rails onto a board: PULL (publicly visible) or PUSH (the actor's
        // OWN standing or OWN view grant — keys are per-User, so membership of
        // this account lifts nothing). (DD 29130754)
        let is_immediately_visible = matches!(
            commission.visibility,
            Visibility::Public | Visibility::Listed
        ) || commission.owner_id == actor_id;

        if !is_immediately_visible && !has_own_view(ports, &actor_id, &commission_id).await? {
            // The same closed door an absent commission gets: `IncorrectRole`
            // would render `403`, an existence oracle over private work.
            return Err(AccountError::NotFound(AccountEntity::Commission));
        }

        column.insert(index, commission_id).map_err(|e| match e {
            WorkflowError::DuplicateCommission | WorkflowError::DuplicateColumnName => {
                AccountError::DuplicateName
            }
            _ => AccountError::Infrastructure(anyhow::anyhow!("Something went wrong")),
        })?;
        let mut uow = ports.database.begin().await?;
        uow.columns().set_commissions(&column).await?;
        uow.commit().await?;

        let commissions = column.into_iter().collect();
        Ok(Output { commissions })
    }
}
