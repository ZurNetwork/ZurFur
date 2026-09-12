use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            ChangelogEntryKind, CommissionId, NewChangelogEntry, NewSeat, SeatKind, SeatLink,
            SeatPrompt, SurfaceAddress, element::SeatId,
        },
        user::UserId,
    },
};
use serde_json::json;

use crate::{
    commission::{CommissionResult, require_owner, seats::Seats},
    ports::WithPorts,
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
    pub async fn declare(&self, cmd: Command, now: DateTimeUtc) -> CommissionResult<Output> {
        let ports = self.ports();
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

        let entry = NewChangelogEntry::event(
            commission.id,
            ChangelogEntryKind::SeatDeclared,
            actor_id,
            json!({
                "kind": seat.kind.as_str(),
                "seat": *seat_id
            }),
            now,
        );

        let mut uow = self.ports().database.begin().await?;
        uow.commissions().declare_seat(&seat).await?;
        uow.changelog().append(&entry).await?;
        uow.commit().await?;
        Ok(Output { seat_id })
    }
}
