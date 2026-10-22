//! What a den write acts with: the act itself, what it declares about itself, and what its
//! type's handler hands back.

use std::fmt;

use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            ChangelogEntryKind, Commission, CommissionId, CommissionRole, NewChangelogEntry,
        },
        level::Level,
        name::NodeName,
        user::UserId,
    },
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
}

/// Where an act is anchored: an existing thing, or a top-level directory a create makes a new
/// thing in.
#[derive(Debug, Clone, PartialEq, Eq)]
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
        match self {
            DenAct::Commission(act) => act.spec(),
        }
    }
}

impl fmt::Debug for DenAct {
    /// The act's variant alone; never its body, which may carry a name.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DenAct::Commission(act) => formatter.debug_tuple("Commission").field(act).finish(),
        }
    }
}

/// A gated kind's domain constructor: its entry on a commission, by an actor, at a time.
pub(in crate::den) type EntryConstructor =
    fn(CommissionId, UserId, DateTimeUtc) -> NewChangelogEntry;

/// A kind the gate writes, paired with its own domain constructor. Its fields are private, so
/// an act names one of its associated consts and never builds one.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GatedEntry {
    kind: ChangelogEntryKind,
    constructor: EntryConstructor,
}

impl GatedEntry {
    /// A create's entry: kind `Created`, by `NewChangelogEntry::created`.
    pub(crate) const CREATED: Self = Self {
        kind: ChangelogEntryKind::Created,
        constructor: NewChangelogEntry::created,
    };

    /// A rename's entry: kind `Renamed`, by `NewChangelogEntry::renamed`.
    pub(crate) const RENAMED: Self = Self {
        kind: ChangelogEntryKind::Renamed,
        constructor: NewChangelogEntry::renamed,
    };

    /// A gated entry of `kind` built by `constructor`, for the tests that hand the gate a
    /// misbuilt one.
    #[cfg(test)]
    pub(in crate::den) fn for_test(
        kind: ChangelogEntryKind,
        constructor: EntryConstructor,
    ) -> Self {
        Self { kind, constructor }
    }

    /// The kind its entry must carry.
    pub(in crate::den) fn kind(self) -> ChangelogEntryKind {
        self.kind
    }

    /// Its entry on `commission`, by `actor`, at `at`, as its domain constructor builds it.
    pub(in crate::den) fn construct(
        self,
        commission: CommissionId,
        actor: UserId,
        at: DateTimeUtc,
    ) -> NewChangelogEntry {
        (self.constructor)(commission, actor, at)
    }
}

/// Which gated entry the gate writes once an act lands: one on every landing, or one only when
/// the act changed its node.
#[derive(Debug, Clone, Copy)]
pub(crate) enum EntryRule {
    /// One entry, whatever the act changed.
    Always(GatedEntry),
    /// One entry when the act changed its node; none when it changed nothing.
    OnChange(GatedEntry),
}

impl EntryRule {
    /// The gated entry an act that `changed` its node writes, or `None`.
    pub(crate) fn entry(self, changed: bool) -> Option<GatedEntry> {
        match self {
            Self::Always(gated) => Some(gated),
            Self::OnChange(gated) => changed.then_some(gated),
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

/// Why a type's handler failed: a store failure under it.
#[derive(Debug, thiserror::Error)]
pub(crate) enum HandlerError {
    /// A store under the handler failed.
    #[error("a store under the handler failed")]
    Store(#[from] anyhow::Error),
}

#[cfg(test)]
mod tests;
