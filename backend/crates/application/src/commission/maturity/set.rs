use crate::LazyUnit;
use crate::use_case;
use domain::elements::{
    commission::CommissionId,
    maturity::{self, MaturityRating},
    user::UserId,
};

use crate::{
    Ports,
    commission::{CommissionResult, maturity::Maturity, require_owner},
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub maturity_rating: MaturityRating,
    pub graphic: bool,
}
pub struct Output;

impl Maturity<'_> {
    #[use_case]
    pub async fn run(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
    ) -> CommissionResult<Output> {
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
        let uow = uow.open().await?;
        uow.commissions()
            .set_maturity(&commission.id, maturity)
            .await?;
        Ok(Output)
    }
}
