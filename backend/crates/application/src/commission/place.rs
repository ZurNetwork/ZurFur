use domain::{
    datetime::DateTimeUtc,
    elements::{account::AccountId, commission::CommissionId, user::UserId},
};

use crate::commission::{CommissionError, CommissionResult, Commissions};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub account_id: AccountId,
}
pub struct Output;

impl Commissions<'_> {
    pub async fn place(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
            account_id,
        } = cmd;
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .filter(|c| c.is_owned_by(&actor_id))
            .ok_or(CommissionError::CommissionNotFound)?;

        let account = ports
            .accounts
            .find(&account_id)
            .await?
            .ok_or(CommissionError::AccountNotFound)?;

        let mut uow = self.ports().database.begin().await?;
        uow.commissions()
            .place(&commission.id, &account.id, &actor_id, now)
            .await?;
        uow.commit().await?;

        Ok(Output)
    }
}
