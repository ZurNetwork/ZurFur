use crate::LazyUnit;
use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, CommissionTitle, NewChangelogEntry},
        text::StoredText,
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{CommissionResult, CommissionValues, Commissions, require_owner},
    common_error::CommonError,
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub title: CommissionTitle,
}

impl Commissions<'_> {
    /// Rename the commission's node and answer the commission as stored. Owner-only through
    /// `require_owner`; the title passed the Den's name rule when it was parsed. A rename to the
    /// stored name writes nothing, and a real one appends a `renamed` entry naming neither
    /// title, in the same unit. The level answered is the node's, read before the unit opens.
    #[use_case]
    pub async fn rename(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<CommissionValues> {
        let Command {
            actor_id,
            commission_id,
            title,
        } = cmd;
        let commission = require_owner(ports, &commission_id, &actor_id).await?;
        let summary = ports
            .commissions
            .find_summary(&commission.id)
            .await?
            .ok_or_else(CommonError::commission_not_found)?;

        let uow = uow.open().await?;
        let mut commissions = uow.commissions();
        let changed = commissions.set_title(&commission.id, &title).await?;
        // Deleted since `require_owner`: the same not-found, and the unit rolls back.
        let stored = commissions
            .find(&commission.id)
            .await?
            .ok_or_else(CommonError::commission_not_found)?;
        drop(commissions);
        if changed {
            let entry = NewChangelogEntry::renamed(commission.id, actor_id, now);
            uow.changelog().append(&entry).await?;
        }
        // Renamed or already so named, the node now holds `title`.
        let name = StoredText::from(title);
        Ok(CommissionValues::from_parts(
            stored,
            name,
            summary.visibility,
        ))
    }
}

#[cfg(test)]
mod tests;
