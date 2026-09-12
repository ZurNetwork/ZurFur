use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, DeadlineStatus},
        user::UserId,
    },
};

use crate::{
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
    ///
    /// `Delayed` is the only value a hand may set: `Late` is the system's word,
    /// written by the deadline sweep, and asking for it is a malformed request
    /// rather than a permission problem. Re-flagging an already-Delayed
    /// commission is an idempotent no-op.
    pub async fn set(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
            status,
        } = cmd;
        match status {
            DeadlineStatus::Delayed => {}
            DeadlineStatus::Late => return Err(CommissionError::InvalidStateRequested),
        }

        super::apply(self.ports(), &commission_id, actor_id, Some(status), now).await?;
        Ok(Output)
    }
}
