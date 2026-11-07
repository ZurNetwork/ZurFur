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
pub struct Output {
    pub commission_id: CommissionId,
}

impl Commissions<'_> {
    /// Return the commission to the active views. Owner-only through
    /// `require`; un-archiving a commission that is not archived is a
    /// no-op.
    #[use_case]
    pub async fn unarchive(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        require(ports, &commission_id, &actor_id, CommissionRole::Owner).await?;

        let entry: NewChangelogEntry = NewChangelogEntry::event(
            commission_id,
            OpenKind::Unarchived,
            actor_id,
            json!({}),
            now,
        )
        .into();

        let uow = uow.open().await?;
        let mut commissions = uow.commissions();
        // `None` clears the stamp; setting it here would archive on the
        // un-archive path.
        let moved = commissions.set_archived(&commission_id, None).await?;
        drop(commissions);
        if moved {
            uow.changelog().append(&entry).await?;
        }
        Ok(Output { commission_id })
    }
}
