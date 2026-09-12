use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, NewChangelogEntry},
        user::UserId,
    },
};
use serde_json::json;

use crate::commission::{CommissionResult, Commissions, require_owner};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}
pub struct Output {
    pub commission_id: CommissionId,
}

impl Commissions<'_> {
    /// Return the commission to the active views — the explicit owner act that
    /// mirrors [`archive`](Commissions::archive).
    ///
    /// Owner-only through the same [`require_owner`] gate, and keyed on the same
    /// real transition: un-archiving a commission that is not archived is an
    /// idempotent no-op with nothing appended.
    pub async fn unarchive(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        let commission = require_owner(ports, &commission_id, &actor_id).await?;

        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::Unarchived,
            actor_id,
            json!({ "title": commission.title.as_str() }),
            now,
        );

        let mut uow = ports.database.begin().await?;
        let mut commissions = uow.commissions();
        // `None` clears the stamp. Setting it again here would archive on the
        // un-archive path — which is exactly what shipped.
        let moved = commissions.set_archived(&commission.id, None).await?;
        drop(commissions);
        if moved {
            uow.changelog().append(&entry).await?;
        }
        uow.commit().await?;
        Ok(Output {
            commission_id: commission.id,
        })
    }
}
