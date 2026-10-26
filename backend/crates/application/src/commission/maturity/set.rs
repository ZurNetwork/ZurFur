use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            AsOwner, CommissionId, CommissionRefusal, CommissionState, MaturitySet, rate_maturity,
        },
        maturity::{Maturity, MaturityRating},
        user::UserId,
    },
};

use crate::{
    Ports,
    commission::{
        CommissionResult, act::CommissionAct, den::CommissionPlace, maturity::Maturity as Rating,
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
    pub maturity_rating: MaturityRating,
    pub graphic: bool,
}

/// The act landed, or there was nothing to change.
#[derive(Debug)]
pub struct Output;

/// The rating: the commission it is anchored on and the rating it gives. The den writes the
/// commission its grant names, never the act's id.
pub(crate) struct Act {
    /// The commission it is anchored on.
    pub(crate) commission: CommissionId,
    /// The rating it gives, Graphic flag included.
    pub(crate) to: Maturity,
}

impl Act {
    /// A change to the commission's fields; it needs the role its capability's marker needs,
    /// the Owner.
    pub(crate) fn spec(&self) -> ActSpec {
        let place = Place::Commission(CommissionPlace::Commission(self.commission));
        ActSpec {
            word: "set_maturity",
            den_type: DenType::Commission,
            anchor: Anchor::Thing(place),
            needs: needs::<Self>(),
            shape: ActShape::Change,
        }
    }
}

impl Handler for Act {
    type Role = AsOwner;

    type Checked = MaturitySet;

    const ENTRY: EntryRule<MaturitySet> = EntryRule::OnChange(GatedEntry::MATURITY_RATED);

    /// The rating rule on the stored state: the rating, or nothing when it is held. No state
    /// refuses it.
    fn precondition(
        &self,
        facts: &CommissionState,
        _now: DateTimeUtc,
    ) -> Result<Option<MaturitySet>, CommissionRefusal> {
        Ok(rate_maturity(facts, self.to))
    }

    /// Writes the checked rating, then answers the commission as the unit holds it.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError> {
        lent.set_maturity().await?;
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

impl Rating<'_> {
    /// Rate (or re-rate) the commission through the Den's write, Owner-only. Replace-only: no
    /// call clears a rating. Giving the rating and Graphic flag it holds writes nothing and
    /// records nothing; a real rating appends a `maturity_rated` entry in the same unit.
    #[use_case]
    pub async fn run(
        &self,
        #[ports] ports: &Ports,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<Output> {
        let Command {
            actor_id,
            commission_id,
            maturity_rating,
            graphic,
        } = cmd;
        let to = Maturity {
            rating: maturity_rating,
            graphic,
        };
        let act = Act {
            commission: commission_id,
            to,
        };
        let command = write::Command {
            actor: actor_id.into(),
            act: DenAct::Commission(CommissionAct::SetMaturity(act)),
        };
        Den::from(ports).write(command, now).await?;
        Ok(Output)
    }
}

#[cfg(test)]
mod tests;
