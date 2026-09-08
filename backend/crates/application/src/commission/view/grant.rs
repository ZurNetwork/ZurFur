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

        // Authority is settled before the unit of work opens, because
        // `provision` is a WRITE keyed to a DID the caller names: running it
        // first let an unauthorized caller intern a User row for a third party
        // and, worse, tell the outcomes apart — a DID already held by another
        // kind of actor answered `409 did_belongs_to_another_actor` where a
        // free DID reached the closed door's `404`, an actor-class oracle over
        // a commission id anyone can invent.
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
