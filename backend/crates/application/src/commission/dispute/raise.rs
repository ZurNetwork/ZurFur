use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            AsParticipant, CommissionId, CommissionRefusal, CommissionState, Dispute, raise_dispute,
        },
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{CommissionResult, act::CommissionAct, den::CommissionPlace, dispute},
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

/// The dispute is raised.
#[derive(Debug)]
pub struct Output;

/// The dispute's raise: the commission it is anchored on. The den writes the commission its
/// grant names, never the act's id.
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
            word: "raise_dispute",
            den_type: DenType::Commission,
            anchor: Anchor::Thing(place),
            needs: needs::<Self>(),
            shape: ActShape::Change,
        }
    }
}

impl Handler for Act {
    type Role = AsParticipant;

    type Checked = Dispute;

    const ENTRY: EntryRule<Dispute> = EntryRule::OnChange(GatedEntry::DISPUTED);

    /// The dispute rule on the stored state at the gate's time: the dispute, from the step it
    /// holds, or refused while already Disputed.
    fn precondition(
        &self,
        facts: &CommissionState,
        now: DateTimeUtc,
    ) -> Result<Option<Dispute>, CommissionRefusal> {
        raise_dispute(facts, now).map(Some)
    }

    /// Writes the checked dispute, then answers the commission as the unit holds it.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError> {
        lent.set_dispute().await?;
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

impl dispute::Dispute<'_> {
    /// Raise a dispute on the commission through the Den's write, as any Participant, from
    /// whatever step it holds, Draft included: it stands Disputed, keeping the step it left
    /// and when the dispute began, and a `lifecycle_moved` entry lands in the same unit.
    /// Refused while already Disputed.
    #[use_case]
    pub async fn raise(
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
            acts: vec![DenAct::Commission(CommissionAct::RaiseDispute(act))],
        };
        Den::from(ports).write(command, now).await?;
        Ok(Output)
    }
}

#[cfg(test)]
mod tests;
