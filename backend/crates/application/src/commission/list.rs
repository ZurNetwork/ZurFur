use domain::elements::{commission::Commission, user::UserId};
use macros::use_case;

use crate::{
    Ports,
    commission::{CommissionResult, Commissions},
};
pub struct Command {
    pub user_id: UserId,
}

pub struct Output {
    pub commissions: Vec<Commission>,
}

impl Commissions<'_> {
    #[use_case]
    pub async fn list(&self, #[ports] ports: &Ports, cmd: Command) -> CommissionResult<Output> {
        let Command { user_id } = cmd;
        Ok(Output {
            commissions: ports.commissions.list_owned_by(&user_id).await?,
        })
    }
}
