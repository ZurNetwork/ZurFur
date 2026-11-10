use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            AsParticipant, CommissionId, CommissionRefusal, CommissionState, DelayedSet,
            set_delayed,
        },
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{
        CommissionResult, act::CommissionAct, deadline::status::Status, den::CommissionPlace,
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

/// The Delayed flag raised: the commission it is anchored on.
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
            word: "set_delayed",
            den_type: DenType::Commission,
            anchor: Anchor::Thing(place),
            needs: needs::<Self>(),
            shape: ActShape::Change,
        }
    }
}

impl Handler for Act {
    type Role = AsParticipant;

    type Checked = DelayedSet;

    const ENTRY: EntryRule<DelayedSet> = EntryRule::OnChange(GatedEntry::DELAYED_SET);

    /// The Delayed rule on the stored state at `now`: the set, nothing when the flag is raised,
    /// or refused while Late and with no deadline. Allowed while Disputed.
    fn precondition(
        &self,
        facts: &CommissionState,
        now: DateTimeUtc,
    ) -> Result<Option<DelayedSet>, CommissionRefusal> {
        set_delayed(facts, now)
    }

    /// Writes the checked set, then answers the commission as the unit holds it.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError> {
        lent.set_deadline_status().await?;
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

impl Status<'_> {
    /// Flag the commission as slipping through the Den's write, as any Participant. Refused
    /// while Late (derived at `now`) and on a commission with no deadline; re-flagging writes
    /// nothing. A real set appends a `delayed` entry in the same unit.
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
        } = cmd;
        let act = Act {
            commission: commission_id,
        };
        let command = write::Command {
            actor: actor_id.into(),
            acts: vec![DenAct::Commission(CommissionAct::SetDelayed(act))],
        };
        Den::from(ports).write(command, now).await?;
        Ok(Output)
    }
}

#[cfg(test)]
mod tests;
