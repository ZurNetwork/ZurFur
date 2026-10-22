//! What a den write acts with: the act itself, what it declares about itself, and what its
//! type's handler hands back.

use domain::elements::{
    commission::{ChangelogEntryKind, CommissionRole},
    level::Level,
    name::NodeName,
};

use crate::den::{DenType, Place, TopLevel};

/// Every act the den's write use case carries. A typed call hands over its act and holds no
/// unit; the den matches over this enum exhaustively, so a new act is a compile error until
/// every step names it.
#[derive(Debug)]
pub(crate) enum DenAct {
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

/// One entry a create makes inside its node: a fixed word as its key, its type, its name and
/// the level its creator picked.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "only the test probe builds one until the next slice's acts"
    )
)]
pub(crate) struct BornEntry {
    /// Its key: one of the Den's fixed words, never request text.
    pub(crate) word: &'static str,
    /// Its type.
    pub(crate) den_type: DenType,
    /// Its name.
    pub(crate) name: NodeName,
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
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "only the test probe builds one until the next slice's acts"
    )
)]
pub(crate) struct ActSpec {
    /// Its word, for the engine's log.
    pub(crate) word: &'static str,
    /// The type of the node it acts on or makes.
    pub(crate) den_type: DenType,
    /// Where it is anchored.
    pub(crate) anchor: Anchor,
    /// The least role it needs on an existing thing; a create needs none.
    pub(crate) needs: CommissionRole,
    /// What it does to the tree.
    pub(crate) shape: ActShape,
}

impl DenAct {
    /// What this act declares about itself.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the den's write use case calls it; no driver calls that until the next slice"
        )
    )]
    pub(crate) fn spec(&self) -> ActSpec {
        match *self {
            #[cfg(test)]
            DenAct::Probe(ref probe) => probe.spec(),
        }
    }
}

/// What a handler hands back for the changelog: the entry's kind and payload. It names no
/// commission and no actor: the gate takes both from the capability and the grant.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "only the test probe drafts one until the next slice's acts"
    )
)]
pub(crate) struct EntryDraft {
    /// What act the entry records.
    pub(crate) kind: ChangelogEntryKind,
    /// The kind's parameters.
    pub(crate) payload: serde_json::Value,
}

/// Why a type's handler declined: its own refusal, or a store failure under it.
#[derive(Debug, thiserror::Error)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "only the test probe returns one until the next slice's acts"
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
