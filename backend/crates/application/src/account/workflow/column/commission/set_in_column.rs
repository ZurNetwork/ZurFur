use crate::LazyUnit;
use crate::use_case;
use domain::elements::{
    commission::{CommissionId, CommissionRole, CommissionStanding},
    level::Level,
    user::UserId,
    workflow::{ColumnId, LexOrdering, WorkflowError},
};

use crate::{
    Ports,
    account::{AccountEntity, AccountError, AccountResult, require_live_account},
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

/// Whether the commission shows on a board without the actor's own view: its owner's, or
/// at a Public or Listed level.
fn is_immediately_visible(standing: &CommissionStanding) -> bool {
    matches!(standing.level, Level::Public | Level::Listed)
        || standing.role == Some(CommissionRole::Owner)
}

/// Whether `actor_id` sees the commission `standing` was read for through their own role or
/// their own view grant. Reads the grant whether or not the commission was found, so a
/// made-up id and a stranger's commission run the same statements.
async fn has_own_view(
    ports: &Ports,
    actor_id: &UserId,
    commission_id: &CommissionId,
    standing: Option<&CommissionStanding>,
) -> Result<bool, AccountError> {
    let granted = ports
        .commissions
        .view_grant(commission_id, actor_id)
        .await?
        .is_some();
    let Some(standing) = standing else {
        return Ok(false);
    };
    Ok(standing.role.is_some() || granted)
}

impl Commissions<'_> {
    #[use_case]
    pub async fn insert_in_column(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
    ) -> AccountResult<Output> {
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
            .await?
            .ok_or(AccountError::NotFound(AccountEntity::Workflow))?;

        require_live_account(ports, &account_id).await?;

        // Any member may put a card on their account's board.
        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .ok_or(AccountError::NotAMember)?;

        let standing = ports
            .commissions
            .standing(&commission_id, &actor_id)
            .await?;

        // Two rails onto a board: PULL (publicly visible) or PUSH (the actor's
        // OWN standing or OWN view grant — keys are per-User, so membership of
        // this account lifts nothing).
        let visible = standing.as_ref().is_some_and(is_immediately_visible);

        if !visible && !has_own_view(ports, &actor_id, &commission_id, standing.as_ref()).await? {
            // The same closed door an absent commission gets: `IncorrectRole`
            // would render `403`, an existence oracle over private work.
            return Err(AccountError::NotFound(AccountEntity::Commission));
        }

        column.insert(index, commission_id).map_err(|e| match e {
            WorkflowError::DuplicateCommission | WorkflowError::DuplicateColumnName => {
                AccountError::DuplicateName
            }
            WorkflowError::IndexOutOfRange(index) => AccountError::IndexOutOfRange(index),
            other => {
                AccountError::Infrastructure(anyhow::anyhow!("the board refused the card: {other}"))
            }
        })?;
        let uow = uow.open().await?;
        uow.columns().set_commissions(&column).await?;

        let commissions = column.into_iter().collect();
        Ok(Output { commissions })
    }
}
