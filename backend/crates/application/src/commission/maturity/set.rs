use domain::elements::{
    commission::CommissionId,
    maturity::{self, MaturityRating},
    user::UserId,
};

use crate::{
    commission::{CommissionResult, maturity::Maturity, require_owner},
    ports::WithPorts,
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub maturity_rating: MaturityRating,
    pub graphic: bool,
}
pub struct Output;

impl Maturity<'_> {
    pub async fn run(&self, cmd: Command) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
            maturity_rating,
            graphic,
        } = cmd;
        let commission = require_owner(ports, &commission_id, &actor_id).await?;

        let maturity = maturity::Maturity {
            graphic,
            rating: maturity_rating,
        };
        let mut uow = self.ports().database.begin().await?;
        uow.commissions()
            .set_maturity(&commission.id, maturity)
            .await?;
        uow.commit().await?;
        Ok(Output)
    }
}
