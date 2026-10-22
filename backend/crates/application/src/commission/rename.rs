use std::convert::Infallible;

use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{AsOwner, CommissionId, CommissionRefusal, CommissionState, CommissionTitle},
        user::UserId,
    },
};

use crate::{
    commission::{
        CommissionResult, CommissionValues, Commissions, act::CommissionAct, den::CommissionPlace,
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
    pub title: CommissionTitle,
}

/// The rename act: the commission it is anchored on and the name its node is given. The den
/// writes the node its grant names, never the act's id.
pub(crate) struct Act {
    /// The commission it is anchored on.
    pub(crate) commission: CommissionId,
    /// The name its node is given.
    pub(crate) title: CommissionTitle,
}

impl Act {
    /// A new name for the commission's node; it needs the role its capability's marker needs,
    /// the Owner.
    pub(crate) fn spec(&self) -> ActSpec {
        let place = Place::Commission(CommissionPlace::Commission(self.commission));
        let shape = ActShape::Rename {
            name: self.title.clone().into(),
        };
        ActSpec {
            word: "rename",
            den_type: DenType::Commission,
            anchor: Anchor::Thing(place),
            needs: needs::<Self>(),
            shape,
        }
    }
}

impl Handler for Act {
    type Role = AsOwner;

    type Checked = Infallible;

    const ENTRY: EntryRule<Infallible> = EntryRule::OnChange(GatedEntry::RENAMED);

    /// No field changes: the gate writes the node's name, so the rule checks nothing.
    fn precondition(
        &self,
        _facts: &CommissionState,
        _now: DateTimeUtc,
    ) -> Result<Option<Infallible>, CommissionRefusal> {
        Ok(None)
    }

    /// Answers the renamed commission as the unit holds it; the gate wrote its node's name.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError> {
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

impl Commissions<'_> {
    /// Rename the commission's node through the Den's write, Owner-only, and answer it as the
    /// write left it. A rename to the stored name writes nothing and records nothing; a real
    /// one appends a `renamed` entry naming neither title, in the same unit.
    #[use_case]
    pub async fn rename(
        &self,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<CommissionValues> {
        let Command {
            actor_id,
            commission_id,
            title,
        } = cmd;
        let act = Act {
            commission: commission_id,
            title,
        };
        let command = write::Command {
            actor: actor_id,
            act: DenAct::Commission(CommissionAct::Rename(act)),
        };
        let written = Den::from(self.ports()).write(command, now).await?;
        let values = CommissionValues::answered(written, now)?;
        Ok(values)
    }
}

#[cfg(test)]
mod tests;
