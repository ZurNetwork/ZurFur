use domain::elements::{commission::Commission, user::UserId};

use crate::commission::{CommissionResult, Commissions};
pub struct Command {
    pub user_id: UserId,
}

pub struct Output {
    pub commissions: Vec<Commission>,
}

impl Commissions<'_> {
    pub async fn list(&self, cmd: Command) -> CommissionResult<Output> {
        let Command { user_id } = cmd;
        Ok(Output {
            commissions: self.ports().commissions.list_owned_by(&user_id).await?,
        })
    }
}
