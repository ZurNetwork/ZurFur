use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, ElementId, NewSlot, SlotTitle, SurfaceAddress, TabId},
        user::UserId,
    },
    ports::Unit,
};
use macros::use_case;

use crate::{
    Ports,
    commission::{CommissionError, CommissionResult, slots::Slots},
    ports::WithPorts,
};

pub struct SlotBody {
    pub tab: TabId,
    pub surface: SurfaceAddress,
    pub title: SlotTitle,
    pub notes: Option<String>,
}
pub struct Command {
    pub user_id: UserId,
    pub commission_id: CommissionId,
    pub slots: Vec<SlotBody>,
}
pub struct Output {
    pub slot_ids: Vec<ElementId>,
}

impl Slots<'_> {
    #[use_case]
    pub async fn declare(
        &self,
        #[ports] ports: &Ports,
        #[unit] uow: Unit<'_>,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            user_id,
            commission_id,
            slots,
        } = cmd;

        if !ports
            .commissions
            .is_participant(&commission_id, &user_id)
            .await?
        {
            return Err(CommissionError::NotAMember);
        }
        let slots: Vec<NewSlot> = slots
            .into_iter()
            .map(|s| {
                // Notes normalize here so every driver inherits it: trimmed,
                // and blank-once-trimmed stored as absent, never "".
                let notes = s
                    .notes
                    .as_deref()
                    .map(str::trim)
                    .filter(|notes| !notes.is_empty())
                    .map(str::to_owned);
                NewSlot::contributed_at(
                    commission_id,
                    s.surface.clone(),
                    s.title.clone(),
                    notes,
                    user_id.clone(),
                    now,
                )
            })
            .collect();

        let slot_ids: Vec<ElementId> = slots.iter().map(|s| s.id).collect();
        uow.commissions().declare_slots(&slots).await?;
        Ok(Output { slot_ids })
    }
}
