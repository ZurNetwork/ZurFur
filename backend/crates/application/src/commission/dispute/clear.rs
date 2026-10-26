use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            AsParticipant, CommissionId, CommissionRefusal, CommissionState, DisputeClear,
            clear_dispute,
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

/// The dispute is cleared.
#[derive(Debug)]
pub struct Output;

/// The dispute's clear: the commission it is anchored on. It takes no step: it returns the
/// commission to the step its dispute left. The den writes the commission its grant names,
/// never the act's id.
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
            word: "clear_dispute",
            den_type: DenType::Commission,
            anchor: Anchor::Thing(place),
            needs: needs::<Self>(),
            shape: ActShape::Change,
        }
    }
}

impl Handler for Act {
    type Role = AsParticipant;

    type Checked = DisputeClear;

    const ENTRY: EntryRule<DisputeClear> = EntryRule::OnChange(GatedEntry::DISPUTE_CLEARED);

    /// The dispute rule on the stored state: the clear, back to the step the dispute left, or
    /// refused when it isn't Disputed.
    fn precondition(
        &self,
        facts: &CommissionState,
        _now: DateTimeUtc,
    ) -> Result<Option<DisputeClear>, CommissionRefusal> {
        clear_dispute(facts).map(Some)
    }

    /// Writes the checked clear, then answers the commission as the unit holds it.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError> {
        lent.clear_dispute().await?;
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

impl dispute::Dispute<'_> {
    /// Clear the commission's dispute through the Den's write, as any Participant: it returns
    /// to exactly the step it left, Draft included, its deadline's clock running again, and a
    /// `lifecycle_moved` entry lands in the same unit. Refused when it isn't Disputed.
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
            acts: vec![DenAct::Commission(CommissionAct::ClearDispute(act))],
        };
        Den::from(ports).write(command, now).await?;
        Ok(Output)
    }
}

#[cfg(test)]
mod tests;
