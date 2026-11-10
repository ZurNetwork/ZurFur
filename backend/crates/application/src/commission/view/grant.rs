use crate::LazyUnit;
use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, CommissionRole, GrantLevel, NewChangelogEntry, OpenKind},
        user::UserId,
    },
};
use serde_json::json;

use crate::{
    Ports,
    commission::{CommissionResult, require_or_not_found, view::View},
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
    #[use_case]
    pub async fn grant(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            target_user_id,
            commission_id,
            level,
        } = cmd;

        // Authority before `provision`: it is a WRITE keyed to a caller-named
        // DID, and its refusals are distinguishable (an actor-class oracle).
        require_or_not_found(ports, &commission_id, &actor_id, CommissionRole::Owner).await?;

        let uow = uow.open().await?;
        let target_user = uow.users().provision(target_user_id.did()).await?;

        let entry: NewChangelogEntry = NewChangelogEntry::event(
            commission_id,
            OpenKind::ViewGrantIssued,
            actor_id,
            json!({
                "commission_id": commission_id,
                "user": target_user.id,
                "level": level.to_string()
            }),
            now,
        )
        .into();

        #[expect(
            clippy::disallowed_methods,
            reason = "the ungated view-grant site, held until its slice routes it through the den's write gate"
        )]
        uow.commissions()
            .grant_view(&commission_id, &target_user.id, level)
            .await?;
        uow.changelog().append(&entry).await?;

        Ok(Output)
    }
}
