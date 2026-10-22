//! What a den write acts with: the act itself, what it declares about itself, and what its
//! type's handler hands back.

use std::fmt;

use domain::elements::{
    commission::{Commission, CommissionRole},
    level::Level,
    name::NodeName,
};

use crate::{
    commission::act::CommissionAct,
    den::{DenType, Place, TopLevel},
};

/// Every act the den's write use case carries. A typed call hands over its act and holds no
/// unit; the den matches over this enum exhaustively, so a new act is a compile error until
/// every step names it.
pub(crate) enum DenAct {
    /// A commission's act.
    Commission(CommissionAct),
    /// The test-only probe, whose every knob a test sets.
    #[cfg(test)]
    Probe(crate::den::probe::Probe),
}

/// Where an act is anchored: an existing thing, or a top-level directory a create makes a new
/// thing in.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "only the test probe builds one until the next slice's acts"
    )
)]
pub(crate) enum Anchor {
    /// An existing thing, at its place.
    Thing(Place),
    /// A top-level directory; only a create may carry it.
    TopLevel(TopLevel),
}

/// One entry a create makes inside its node: a fixed word as its key and its name, its type,
/// and the level its creator picked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BornEntry {
    /// Its key and its name: one of the Den's fixed words, never request text.
    pub(crate) word: &'static str,
    /// Its type.
    pub(crate) den_type: DenType,
    /// The level its creator picked; `None` means Private.
    pub(crate) level: Option<Level>,
}

/// What an act does to the tree, in the den's own values: the builder turns it into Atta's.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "only the test probe builds one until the next slice's acts"
    )
)]
pub(crate) enum ActShape {
    /// Makes a new thing named `name`, holding `entries`; its key is the id the den mints.
    Create {
        /// Its name.
        name: NodeName,
        /// The level its creator picked; `None` means Private.
        level: Option<Level>,
        /// The entries made inside it.
        entries: Vec<BornEntry>,
    },
    /// Gives the anchored thing a new name.
    Rename {
        /// Its new name.
        name: NodeName,
    },
    /// Changes the anchored thing's content.
    Change,
}

/// What an act declares about itself: its word, the type it acts on or makes, its anchor, the
/// role it needs on an existing thing, and its shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActSpec {
    /// Its word, for the engine's log.
    pub(crate) word: &'static str,
    /// The type of the node it acts on or makes.
    pub(crate) den_type: DenType,
    /// Where it is anchored.
    pub(crate) anchor: Anchor,
    /// The least role it needs on an existing thing, from its capability's role; `None` for a
    /// create, which a thing anchor refuses.
    pub(crate) needs: Option<CommissionRole>,
    /// What it does to the tree.
    pub(crate) shape: ActShape,
}

impl DenAct {
    /// What this act declares about itself.
    pub(crate) fn spec(&self) -> ActSpec {
        match *self {
            DenAct::Commission(ref act) => act.spec(),
            #[cfg(test)]
            DenAct::Probe(ref probe) => probe.spec(),
        }
    }
}

impl fmt::Debug for DenAct {
    /// The act's variant alone; never its body, which may carry a name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DenAct::Commission(ref act) => formatter.debug_tuple("Commission").field(act).finish(),
            #[cfg(test)]
            DenAct::Probe(_) => formatter.write_str("Probe"),
        }
    }
}

/// What a handler answers once its act lands: the thing it acted on, as the view read it.
pub(crate) struct Handled {
    /// The commission the act acted on or made.
    pub(crate) answer: Commission,
}

/// What a den write answers beside the node it wrote, by its arm.
pub(crate) enum ArmAnswer {
    /// The commission the act acted on or made.
    Commission(Commission),
}

impl fmt::Debug for ArmAnswer {
    /// The arm and the thing's id; never its owner, whose id is a DID.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArmAnswer::Commission(commission) => formatter
                .debug_tuple("Commission")
                .field(&commission.id)
                .finish(),
        }
    }
}

/// Why a type's handler declined: its own refusal, or a store failure under it.
#[derive(Debug, thiserror::Error)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "only the test probe refuses until the next slice's acts"
    )
)]
pub(crate) enum HandlerError {
    /// The type refused the act.
    #[error("the type refused the act")]
    Refused,
    /// A store under the handler failed.
    #[error("a store under the handler failed")]
    Store(#[from] anyhow::Error),
}
