use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, NewChangelogEntry},
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
}
pub struct Output;

impl View<'_> {
    /// Revokes the target User's view grant, hard-deleting the key. Owner-only;
    /// every other caller gets the closed door's `CommissionNotFound`. Revoking
    /// a key nobody holds succeeds and records nothing.
    pub async fn revoke(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            target_user_id,
            commission_id,
        } = cmd;

        // Authority is settled before the unit of work opens — `provision` is a
        // WRITE keyed to a DID the caller names, and its refusals are
        // distinguishable. See the twin note on `view::grant`.
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
            ChangelogEntryKind::ViewGrantRevoked,
            actor_id,
            json!({
                "target_id": target_user.id,

            }),
            now,
        );

        // Keyed on a real revocation: revoking a grant nobody holds changes
        // nothing, so it records nothing. The changelog is evidence, not a log
        // of attempts.
        let mut commissions = uow.commissions();
        let revoked = commissions
            .revoke_view(&commission.id, &target_user.id)
            .await?;
        drop(commissions);
        if revoked {
            uow.changelog().append(&entry).await?;
        }
        uow.commit().await?;
        Ok(Output)
    }
}
