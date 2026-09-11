use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, GrantLevel, NewChangelogEntry},
        user::UserId,
    },
};
use serde_json::json;

use crate::{
    commission::{CommissionError, CommissionResult, view::View},
    ports::WithPorts,
};

pub struct Command {
    pub actor_id: UserId,
    pub target_user_id: UserId,
    pub commission_id: CommissionId,
    pub level: GrantLevel,
}
pub struct Output;

impl View<'_> {
    /// Issues the target User a view grant at `level`, replacing any key they
    /// already hold, and records the issuance. Owner-only; every other caller
    /// gets the closed door's `CommissionNotFound`.
    pub async fn grant(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            target_user_id,
            commission_id,
            level,
        } = cmd;

        // Authority before `provision`: it is a WRITE keyed to a caller-named
        // DID, and its refusals are distinguishable (an actor-class oracle).
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .filter(|c| c.is_owned_by(&actor_id))
            .ok_or(CommissionError::CommissionNotFound)?;

        let mut uow = self.ports().database.begin().await?;
        let target_user = uow.users().provision(&target_user_id).await?;

        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::ViewGrantIssued,
            actor_id,
            json!({
                "commission_id": commission.id,
                "user": target_user.id,
                "level": level.to_string()
            }),
            now,
        );

        uow.commissions()
            .grant_view(&commission.id, &target_user.id, level)
            .await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;

        Ok(Output)
    }
}
