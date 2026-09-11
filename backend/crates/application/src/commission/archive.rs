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
pub struct Outcome;

impl Commissions<'_> {
    /// Archive the commission — it leaves the active views; the record survives.
    ///
    /// Owner-only through the shared [`require_owner`] gate, so a
    /// non-participant gets the uniform not-found. Archiving an
    /// already-archived commission is an idempotent no-op: the flag write and
    /// the `archived` entry land in one unit of work, and the entry is keyed on
    /// the store reporting a *real* transition — a record of nothing changing
    /// would be noise, not audit.
    pub async fn archive(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Outcome> {
        let ports = self.ports();
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        let commission = require_owner(ports, &commission_id, &actor_id).await?;

        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::Archived,
            actor_id,
            json!({ "title": commission.title.as_str() }),
            now,
        );

        let mut uow = ports.database.begin().await?;
        let mut commissions = uow.commissions();
        let moved = commissions.set_archived(&commission.id, Some(now)).await?;
        drop(commissions);
        if moved {
            uow.changelog().append(&entry).await?;
        }
        uow.commit().await?;
        Ok(Outcome)
    }
}
