use crate::LazyUnit;
use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, CommissionRole, NewChangelogEntry, OpenKind},
        user::UserId,
    },
};
use serde_json::json;

use crate::{
    Ports,
    commission::{CommissionResult, Commissions, require},
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}
pub struct Outcome;

impl Commissions<'_> {
    /// Archive the commission — it leaves the active views, the record
    /// survives. Owner-only through `require`, so a non-participant
    /// gets the uniform not-found. Archiving an already-archived commission is
    /// a no-op; the flag write and its entry land in one unit of work.
    #[use_case]
    pub async fn archive(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Outcome> {
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        require(ports, &commission_id, &actor_id, CommissionRole::Owner).await?;

        let entry: NewChangelogEntry =
            NewChangelogEntry::event(commission_id, OpenKind::Archived, actor_id, json!({}), now)
                .into();

        let uow = uow.open().await?;
        let mut commissions = uow.commissions();
        let moved = commissions.set_archived(&commission_id, Some(now)).await?;
        drop(commissions);
        if moved {
            uow.changelog().append(&entry).await?;
        }
        Ok(Outcome)
    }
}
