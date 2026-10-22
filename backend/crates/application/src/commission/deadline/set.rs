use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            AsParticipant, CommissionId, CommissionRefusal, CommissionState, DeadlineSet,
            schedule_deadline,
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
    pub deadline: DateTimeUtc,
}

/// The act landed, or there was nothing to change.
#[derive(Debug)]
pub struct Output;

/// The deadline set or moved: the commission it is anchored on and the deadline it sets.
/// The den writes the commission its grant names, never the act's id.
pub(crate) struct Act {
    /// The commission it is anchored on.
    pub(crate) commission: CommissionId,
    /// The deadline it sets.
    pub(crate) deadline: DateTimeUtc,
}

impl Act {
    /// A change to the commission's fields; it needs the role its capability's marker needs,
    /// any Participant.
    pub(crate) fn spec(&self) -> ActSpec {
        let place = Place::Commission(CommissionPlace::Commission(self.commission));
        ActSpec {
            word: "set_deadline",
            den_type: DenType::Commission,
            anchor: Anchor::Thing(place),
            needs: needs::<Self>(),
            shape: ActShape::Change,
        }
    }
}

impl Handler for Act {
    type Role = AsParticipant;

    type Checked = DeadlineSet;

    const ENTRY: EntryRule<DeadlineSet> = EntryRule::OnChange(GatedEntry::DEADLINE_SET);

    /// The deadline rule on the stored state: the set, nothing when the deadline is held, or
    /// refused while Disputed.
    fn precondition(
        &self,
        facts: &CommissionState,
        _now: DateTimeUtc,
    ) -> Result<Option<DeadlineSet>, CommissionRefusal> {
        schedule_deadline(facts, self.deadline)
    }

    /// Writes the checked set, then answers the commission as the unit holds it.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError> {
        lent.set_deadline().await?;
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

impl Deadline<'_> {
    /// Set or move the commission's deadline through the Den's write, as any Participant.
    /// Re-setting the deadline it holds writes nothing; a Disputed commission's is refused. A
    /// later deadline than the one held records `deadline_extended`, anything else
    /// `deadline_set`.
    #[use_case]
    pub async fn set(
        &self,
        #[ports] ports: &Ports,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
            deadline,
        } = cmd;
        let act = Act {
            commission: commission_id,
            deadline,
        };
        let command = write::Command {
            actor: actor_id,
            act: DenAct::Commission(CommissionAct::SetDeadline(act)),
        };
        Den::from(ports).write(command, now).await?;
        Ok(Output)
    }
}

#[cfg(test)]
mod tests;
