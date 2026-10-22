use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            AsParticipant, CommissionId, CommissionRefusal, CommissionState, LifecycleMove,
            LifecycleStep, move_lifecycle,
        },
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{
        CommissionResult, act::CommissionAct, den::CommissionPlace, lifecycle::Lifecycle,
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
    pub to: LifecycleStep,
}

/// The move landed, or there was nothing to move.
#[derive(Debug)]
pub struct Output;

/// The lifecycle move: the commission it is anchored on and the step it moves to. The den
/// writes the commission its grant names, never the act's id.
pub(crate) struct Act {
    /// The commission it is anchored on.
    pub(crate) commission: CommissionId,
    /// The step it moves to.
    pub(crate) to: LifecycleStep,
}

impl Act {
    /// A change to the commission's fields; it needs the role its capability's marker needs,
    /// any Participant.
    pub(crate) fn spec(&self) -> ActSpec {
        let place = Place::Commission(CommissionPlace::Commission(self.commission));
        ActSpec {
            word: "move_lifecycle",
            den_type: DenType::Commission,
            anchor: Anchor::Thing(place),
            needs: needs::<Self>(),
            shape: ActShape::Change,
        }
    }
}

impl Handler for Act {
    type Role = AsParticipant;

    type Checked = LifecycleMove;

    const ENTRY: EntryRule<LifecycleMove> = EntryRule::OnChange(GatedEntry::LIFECYCLE_MOVED);

    /// The lifecycle rule on the stored state: the move, nothing when the step is held, or its
    /// refusal.
    fn precondition(
        &self,
        facts: &CommissionState,
        _now: DateTimeUtc,
    ) -> Result<Option<LifecycleMove>, CommissionRefusal> {
        move_lifecycle(facts, self.to.clone())
    }

    /// Writes the checked move, then answers the commission as the unit holds it.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError> {
        lent.set_lifecycle().await?;
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

impl Lifecycle<'_> {
    /// Move the commission's lifecycle step through the Den's write, as any Participant. A
    /// move to the step it holds writes nothing and records nothing; a real one appends a
    /// `lifecycle_moved` entry in the same unit. Refused while Disputed, and into Draft or
    /// Disputed.
    #[use_case]
    pub async fn move_to(
        &self,
        #[ports] ports: &Ports,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
            to,
        } = cmd;
        let act = Act {
            commission: commission_id,
            to,
        };
        let command = write::Command {
            actor: actor_id,
            act: DenAct::Commission(CommissionAct::MoveLifecycle(act)),
        };
        Den::from(ports).write(command, now).await?;
        Ok(Output)
    }
}

#[cfg(test)]
mod tests;
