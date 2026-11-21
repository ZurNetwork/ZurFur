use std::fmt;

use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            AsOwner, CommissionId, CommissionRefusal, CommissionState, ElementId, NewSlot,
            SlotDeclaration, SlotTitle, SurfaceAddress,
        },
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{
        CommissionError, CommissionResult, act::CommissionAct, den::CommissionPlace, slots::Slots,
    },
    den::{
        Den, DenType, Handler, Lent, Place,
        act::{ActShape, ActSpec, Anchor, DenAct, EntryRule, GatedEntry, Handled, HandlerError},
        names, needs, write,
    },
};

/// One Slot a declare asks for: where its carrying element sits, its title and its notes.
pub struct SlotBody {
    pub surface: SurfaceAddress,
    pub title: SlotTitle,
    pub notes: Option<String>,
}

pub struct Command {
    pub user_id: UserId,
    pub commission_id: CommissionId,
    pub slots: Vec<SlotBody>,
}

/// The declared Slots' element ids, in request order.
#[derive(Debug)]
pub struct Output {
    pub slot_ids: Vec<ElementId>,
}

impl fmt::Debug for SlotBody {
    /// Its surface; never its title or notes.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SlotBody")
            .field("surface", &self.surface)
            .finish_non_exhaustive()
    }
}

/// One Slot's declare: the commission it is anchored on, the title its node is named by, and
/// its declaration, under the element id the use case minted. The den writes the commission
/// its grant names, never the act's id.
pub(crate) struct Act {
    /// The commission it is anchored on.
    pub(crate) commission: CommissionId,
    /// The name its node is given.
    pub(crate) title: SlotTitle,
    /// The element it is born as, and what the declare chose.
    pub(crate) declaration: SlotDeclaration,
}

impl Act {
    /// A Slot's node made in the commission's `slots` entry, Private, keyed by its element id;
    /// it needs the role its capability's marker needs, the Owner.
    pub(crate) fn spec(&self) -> ActSpec {
        let place = Place::Commission(CommissionPlace::Commission(self.commission));
        let shape = ActShape::CreateInEntry {
            entry: names::SLOTS,
            entry_type: DenType::CommissionSlots,
            key: self.declaration.element,
            name: self.title.clone().into(),
        };
        ActSpec {
            word: "declare_slots",
            den_type: DenType::Slot,
            anchor: Anchor::Thing(place),
            needs: needs::<Self>(),
            shape,
        }
    }
}

impl fmt::Debug for Act {
    /// The commission and the element; never the title or the notes.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Act")
            .field("commission", &self.commission)
            .field("element", &self.declaration.element)
            .finish_non_exhaustive()
    }
}

impl Handler for Act {
    type Role = AsOwner;

    type Checked = SlotDeclaration;

    const ENTRY: EntryRule<SlotDeclaration> = EntryRule::Always(GatedEntry::SLOT_DECLARED);

    /// The declaration as the act carries it: no rule of the commission's state refuses it.
    fn precondition(
        &self,
        _facts: &CommissionState,
        _now: DateTimeUtc,
    ) -> Result<Option<SlotDeclaration>, CommissionRefusal> {
        Ok(Some(self.declaration.clone()))
    }

    /// Declares the Slot at the node the gate made, refusing a tab or surface the commission
    /// doesn't hold, then answers the commission as the unit holds it.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError> {
        lent.declare_slot().await?.map_err(HandlerError::Refused)?;
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

/// `body` as a declare of one Slot on `commission`, under a freshly minted element id. Notes
/// normalize here so every driver inherits it: trimmed, and blank once trimmed stored as absent.
fn act_of(commission: CommissionId, body: SlotBody) -> Act {
    let SlotBody {
        surface,
        title,
        notes,
    } = body;
    let notes = notes
        .as_deref()
        .map(str::trim)
        .filter(|notes| !notes.is_empty())
        .map(str::to_owned);
    let slot = NewSlot {
        address: surface,
        notes,
    };
    Act {
        commission,
        title,
        declaration: SlotDeclaration::minted(slot),
    }
}

impl Slots<'_> {
    /// Declare Slots on the commission through the Den's write, as its Owner: each Slot's node
    /// in the commission's `slots` entry, its element and satellite, and one `slot_declared`
    /// entry, all in one unit, all or none. Answers the Slots' element ids in request order.
    #[use_case]
    pub async fn declare(
        &self,
        #[ports] ports: &Ports,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            user_id,
            commission_id,
            slots,
        } = cmd;
        if slots.is_empty() {
            return Err(CommissionError::IncorrectContent);
        }
        let acts: Vec<Act> = slots
            .into_iter()
            .map(|body| act_of(commission_id, body))
            .collect();
        let slot_ids = acts.iter().map(|act| act.declaration.element).collect();
        let den_acts = acts
            .into_iter()
            .map(|act| DenAct::Commission(CommissionAct::DeclareSlot(act)))
            .collect();
        let command = write::Command {
            actor: user_id.into(),
            acts: den_acts,
        };
        Den::from(ports).write(command, now).await?;
        Ok(Output { slot_ids })
    }
}

#[cfg(test)]
mod tests;
