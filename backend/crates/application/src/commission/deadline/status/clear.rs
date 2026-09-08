use domain::{
    datetime::DateTimeUtc,
    elements::{commission::CommissionId, user::UserId},
};

use crate::{
    commission::{CommissionResult, deadline::status::Status},
    ports::WithPorts,
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}
pub struct Output;

impl Status<'_> {
    /// Clear the deadline-axis status — the Participant taking their own
    /// slipping flag back down.
    ///
    /// A standing `Late` is refused here as it is on the set side: clearing it
    /// by hand would overrule the system. The honest lever out of Late is
    /// clearing the *deadline*. Clearing an already-clear axis is an idempotent
    /// no-op.
    pub async fn clear(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        super::apply(self.ports(), &commission_id, actor_id, None, now).await?;
        Ok(Output)
    }
}
