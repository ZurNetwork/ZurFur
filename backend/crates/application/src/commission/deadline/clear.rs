use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            AsParticipant, CommissionId, CommissionRefusal, CommissionState, DeadlineCleared,
            clear_deadline,
        },
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{CommissionResult, act::CommissionAct, deadline::Deadline, den::CommissionPlace},
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

/// The deadline cleared: the commission it is anchored on.
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
            word: "clear_deadline",
            den_type: DenType::Commission,
            anchor: Anchor::Thing(place),
            needs: needs::<Self>(),
            shape: ActShape::Change,
        }
    }
}

impl Handler for Act {
    type Role = AsParticipant;

    type Checked = DeadlineCleared;

    const ENTRY: EntryRule<DeadlineCleared> = EntryRule::OnChange(GatedEntry::DEADLINE_CLEARED);

    /// The deadline rule on the stored state: the clear, nothing when no deadline is held, or
    /// refused while Disputed.
    fn precondition(
        &self,
        facts: &CommissionState,
        _now: DateTimeUtc,
    ) -> Result<Option<DeadlineCleared>, CommissionRefusal> {
        clear_deadline(facts)
    }

    /// Writes the checked clear, then answers the commission as the unit holds it.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError> {
        lent.set_deadline().await?;
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

impl Deadline<'_> {
    /// Clear the commission's deadline through the Den's write, as any Participant: the lever
    /// out of a Late standing. Clearing an absent deadline writes nothing; a Disputed
    /// commission's is refused.
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
            actor: actor_id,
            act: DenAct::Commission(CommissionAct::ClearDeadline(act)),
        };
        Den::from(ports).write(command, now).await?;
        Ok(Output)
    }
}

#[cfg(test)]
mod tests;
