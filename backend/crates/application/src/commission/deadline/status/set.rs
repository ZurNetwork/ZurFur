use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, DeadlineStatus},
        user::UserId,
    },
    ports::Unit,
};
use macros::use_case;

use crate::{
    Ports,
    commission::{CommissionError, CommissionResult, deadline::status::Status},
    ports::WithPorts,
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub status: DeadlineStatus,
}
pub struct Output;

impl Status<'_> {
    /// Flag the commission as slipping — the manual Participant act.
    /// `Delayed` is the only value a hand may set; asking for `Late` is a
    /// malformed request, not a permission problem. Re-flagging is a no-op.
    #[use_case]
    pub async fn set(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
            status,
        } = cmd;
        match status {
            DeadlineStatus::Delayed => {}
            DeadlineStatus::Late => return Err(CommissionError::InvalidStateRequested),
        }

        super::apply(ports, uow, &commission_id, actor_id, Some(status), now).await?;
        Ok(Output)
    }
}
