use crate::use_case;
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{ChangelogEntryKind, CommissionTitle, NewCommission},
        maturity::Maturity,
        user::UserId,
    },
    ports::Creator,
};

use crate::{
    commission::{CommissionResult, CommissionValues, Commissions, act::CommissionAct},
    den::{
        Den, DenType, Handler, Lent, TopLevel,
        act::{ActShape, ActSpec, Anchor, BornEntry, DenAct, Handled, HandlerError},
        names, needs, write,
    },
};

pub struct Command {
    pub actor_id: UserId,
    pub title: CommissionTitle,
    pub maturity: Option<Maturity>,
    pub deadline: Option<DateTimeUtc>,
}

/// The create act: the commission's name and what a create may choose. The den mints its id
/// and supplies its node, its owner and its time.
pub(crate) struct Act {
    /// The name its node is given.
    pub(crate) title: CommissionTitle,
    /// Its maturity and its deadline.
    pub(crate) chosen: NewCommission,
}

/// The six entries a commission is made with, each Private, keyed and named by its word.
const ENTRIES: [(&str, DenType); 6] = [
    (names::ATTACHMENTS, DenType::CommissionAttachments),
    (names::PRODUCTS, DenType::CommissionProducts),
    (names::SLOTS, DenType::CommissionSlots),
    (names::CHANGELOG, DenType::CommissionChangelog),
    (names::PARTICIPANTS, DenType::CommissionParticipants),
    (names::BRIEF, DenType::CommissionBrief),
];

impl Act {
    /// A commission made in `/commission`, Private, holding its six entries; it needs the role
    /// its capability's marker needs, which is none.
    pub(crate) fn spec(&self) -> ActSpec {
        let entries = ENTRIES
            .iter()
            .map(|&(word, den_type)| BornEntry {
                word,
                den_type,
                level: None,
            })
            .collect();
        let shape = ActShape::Create {
            name: self.title.clone().into(),
            level: None,
            entries,
        };
        ActSpec {
            word: "create",
            den_type: DenType::Commission,
            anchor: Anchor::TopLevel(TopLevel::Commission),
            needs: needs::<Self>(),
            shape,
        }
    }
}

impl Handler for Act {
    type Role = Creator;

    const ENTRY: ChangelogEntryKind = ChangelogEntryKind::Created;

    /// Creates the commission at the node the gate made, then answers it as the unit holds it.
    async fn handle(&self, lent: &mut Lent<'_, Creator>) -> Result<Handled, HandlerError> {
        lent.create(&self.chosen).await?;
        let answer = lent.lent().await?;
        Ok(Handled { answer })
    }
}

impl Commissions<'_> {
    /// Create a commission owned by the caller through the Den's write, which makes its node,
    /// its entries and its `created` entry in one unit; answer it as the write left it.
    #[use_case]
    pub async fn create(
        &self,
        cmd: Command,
        now: DateTimeUtc,
    ) -> CommissionResult<CommissionValues> {
        let Command {
            actor_id,
            title,
            maturity,
            deadline,
        } = cmd;
        let act = Act {
            title,
            chosen: NewCommission { maturity, deadline },
        };
        let command = write::Command {
            actor: actor_id,
            act: DenAct::Commission(CommissionAct::Create(act)),
        };
        let written = Den::from(self.ports()).write(command, now).await?;
        let values = CommissionValues::try_from(written)?;
        Ok(values)
    }
}

#[cfg(test)]
mod tests;
