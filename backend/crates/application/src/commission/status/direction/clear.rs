use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            AsParticipant, CommissionId, CommissionRefusal, CommissionState, DirectionCleared,
            clear_direction,
        },
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{
        CommissionResult, act::CommissionAct, den::CommissionPlace, status::direction::Direction,
    },
    den::{
        Den, DenType, Handler, Lent, Place,
        act::{ActShape, ActSpec, Anchor, DenAct, EntryRule, GatedEntry, Handled, HandlerError},
        needs, write,
    },
};

pub struct Command {
    pub actor_id: UserId,
    pub commission_id: CommissionId,
}

/// The act landed, or there was nothing to change.
#[derive(Debug)]
pub struct Output;

/// The direction status cleared: the commission it is anchored on.
/// The den writes the commission its grant names, never the act's id.
pub(crate) struct Act {
    /// The commission it is anchored on.
    pub(crate) commission: CommissionId,
}

impl Act {
    /// A change to the commission's fields; it needs the role its capability's marker needs,
    /// any Participant.
    pub(crate) fn spec(&self) -> ActSpec {
        let place = Place::Commission(CommissionPlace::Commission(self.commission));
        ActSpec {
            word: "clear_direction_status",
            den_type: DenType::Commission,
            anchor: Anchor::Thing(place),
            needs: needs::<Self>(),
            shape: ActShape::Change,
        }
    }
}

impl Handler for Act {
    type Role = AsParticipant;

    type Checked = DirectionCleared;

    const ENTRY: EntryRule<DirectionCleared> = EntryRule::OnChange(GatedEntry::STATUS_CLEARED);

    /// The direction rule on the stored state: the clear, or nothing when no status is held. No
    /// state refuses it.
    fn precondition(
        &self,
        facts: &CommissionState,
        _now: DateTimeUtc,
    ) -> Result<Option<DirectionCleared>, CommissionRefusal> {
        Ok(clear_direction(facts))
    }

    /// Writes the checked clear, then answers the commission as the unit holds it.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError> {
        lent.set_direction_status().await?;
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

impl Direction<'_> {
    /// Clear the commission's direction status through the Den's write, as any Participant.
    /// Clearing an absent status writes nothing and records nothing; a real clear appends a
    /// `status_changed` entry in the same unit.
    #[use_case]
    pub async fn clear(
        &self,
        #[ports] ports: &Ports,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
        } = cmd;
        let act = Act {
            commission: commission_id,
        };
        let command = write::Command {
            actor: actor_id.into(),
            act: DenAct::Commission(CommissionAct::ClearDirection(act)),
        };
        Den::from(ports).write(command, now).await?;
        Ok(Output)
    }
}

#[cfg(test)]
mod tests;
