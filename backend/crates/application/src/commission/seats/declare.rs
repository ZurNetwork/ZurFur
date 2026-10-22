use crate::LazyUnit;
use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            CommissionId, NewChangelogEntry, NewSeat, OpenKind, SeatKind, SeatLink, SeatPrompt,
            SurfaceAddress, element::SeatId,
        },
        user::UserId,
    },
};
use serde_json::json;

use crate::{
    Ports,
    commission::{CommissionResult, require_owner, seats::Seats},
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
    pub seat_kind: SeatKind,
    pub prompt: Option<SeatPrompt>,
    pub link: Option<SeatLink>,
    pub surface_address: SurfaceAddress,
}
pub struct Output {
    pub seat_id: SeatId,
}

impl Seats<'_> {
    #[use_case]
    pub async fn declare(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
            seat_kind,
            prompt,
            link,
            surface_address,
        } = cmd;
        let commission = require_owner(ports, &commission_id, &actor_id).await?;

        let seat = NewSeat::contributed_at(
            commission.id,
            surface_address,
            seat_kind,
            prompt,
            link,
            actor_id.clone(),
            now,
        );
        let seat_id = seat.id;

        let entry: NewChangelogEntry = NewChangelogEntry::event(
            commission.id,
            OpenKind::SeatDeclared,
            actor_id,
            json!({
                "kind": seat.kind.as_str(),
                "seat": seat_id.to_string()
            }),
            now,
        )
        .into();

        let uow = uow.open().await?;
        #[expect(
            clippy::disallowed_methods,
            reason = "the ungated seat-declare site, held until its slice routes it through the den's write gate"
        )]
        uow.commissions().declare_seat(&seat).await?;
        uow.changelog().append(&entry).await?;
        Ok(Output { seat_id })
    }
}
