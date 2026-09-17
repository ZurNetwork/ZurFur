use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionId, NewChangelogEntry},
        user::UserId,
    },
    ports::Unit,
};
use macros::use_case;
use serde_json::json;

use crate::{
    Ports,
    commission::{CommissionResult, view::View},
    common_error::{CommonError, NotFoundEntity},
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
    #[use_case]
    pub async fn revoke(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            target_user_id,
            commission_id,
        } = cmd;

        // Authority before `provision` — see the twin note on `view::grant`.
        let commission = ports
            .commissions
            .find(&commission_id)
            .await?
            .filter(|c| c.is_owned_by(&actor_id))
            .ok_or(CommonError::NotFound(NotFoundEntity::Commission))?;

        let target_user = uow.users().provision(target_user_id.did()).await?;

        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::ViewGrantRevoked,
            actor_id,
            json!({
                "target_id": target_user.id,

            }),
            now,
        );

        // Keyed on a real revocation: revoking a grant nobody holds records
        // nothing.
        let mut commissions = uow.commissions();
        let revoked = commissions
            .revoke_view(&commission.id, &target_user.id)
            .await?;
        drop(commissions);
        if revoked {
            uow.changelog().append(&entry).await?;
        }
        Ok(Output)
    }
}
