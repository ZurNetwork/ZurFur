//! What a den write acts with: the act itself, what it declares about itself, and what its
//! type's handler hands back.

use std::fmt;

use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            ChangelogEntryKind, Commission, CommissionId, CommissionRole, DeadlineCleared,
            DeadlineSet, DelayedCleared, DelayedSet, DirectionCleared, DirectionSet, Dispute,
            DisputeClear, ElementId, LateMark, LifecycleMove, MaturitySet, NewChangelogEntry,
            NewCommission, SlotDeclaration,
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
    /// Makes a new node named `name` in the anchored thing's fixed entry `entry`, keyed by the
    /// id of the thing it stands for; it is born Private.
    CreateInEntry {
        /// The entry it is made in: one of the Den's fixed words, never request text.
        entry: &'static str,
        /// The entry's type.
        entry_type: DenType,
        /// Its key: the id of the thing it stands for.
        key: ElementId,
        /// Its name.
        name: NodeName,
    },
    /// Gives the anchored thing a new name.
    Rename {
        /// Its new name.
        name: NodeName,
    },
    /// Changes the anchored thing's fields; its node keeps its rows.
    Change,
}

/// Who an act needs: anyone signed in, to make a new thing; a user holding a role on an existing
/// thing; or the system, with no user behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Needs {
    /// A create, which a top-level anchor carries; a thing anchor refuses it.
    Create,
    /// The least role a user needs on an existing thing.
    Role(CommissionRole),
    /// The system's own act, which no user may make.
    System,
}

/// What an act declares about itself: its word, the type it acts on or makes, its anchor, who
/// it needs, and its shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActSpec {
    /// Its word, for the engine's log.
    pub(crate) word: &'static str,
    /// The type of the node it acts on or makes.
    pub(crate) den_type: DenType,
    /// Where it is anchored.
    pub(crate) anchor: Anchor,
    /// Who it needs, from its capability's role: a create, a user's role, or the system.
    pub(crate) needs: Needs,
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

/// A gated kind's domain constructor: its entry on a commission, by an actor, at a time, with
/// no change, or with the act's checked change `C`, or with the checked change and maybe no
/// entry; or the system's entry, by no actor, with the checked change.
pub(in crate::den) enum EntryConstructor<C> {
    /// A constructor that takes no change.
    Bare(fn(CommissionId, UserId, DateTimeUtc) -> NewChangelogEntry),
    /// A constructor that takes the act's checked change.
    Change(fn(CommissionId, UserId, &C, DateTimeUtc) -> NewChangelogEntry),
    /// A constructor that takes the act's checked change and may find nothing in it to record.
    Optional(fn(CommissionId, UserId, &C, DateTimeUtc) -> Option<NewChangelogEntry>),
    /// A constructor of the system's entry, which takes no actor and the act's checked change.
    System(fn(CommissionId, &C, DateTimeUtc) -> NewChangelogEntry),
}

impl<C> fmt::Debug for EntryConstructor<C> {
    /// Which shape it takes.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bare(_) => formatter.write_str("Bare"),
            Self::Change(_) => formatter.write_str("Change"),
            Self::Optional(_) => formatter.write_str("Optional"),
            Self::System(_) => formatter.write_str("System"),
        }
    }
}

/// The kinds a gated entry may build, paired with its own domain constructor, over the act's
/// checked change `C`. Its fields are private, so an act names one of its associated consts
/// and never builds one.
#[derive(Debug)]
pub(crate) struct GatedEntry<C> {
    kinds: &'static [ChangelogEntryKind],
    constructor: EntryConstructor<C>,
}

impl<C> GatedEntry<C> {
    /// A create's entry: kind `Created`, by `NewChangelogEntry::created`, with no change.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const CREATED: Self = Self {
        kinds: &[ChangelogEntryKind::Created],
        constructor: EntryConstructor::Bare(NewChangelogEntry::created),
    };

    /// A rename's entry: kind `Renamed`, by `NewChangelogEntry::renamed`, with no change.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const RENAMED: Self = Self {
        kinds: &[ChangelogEntryKind::Renamed],
        constructor: EntryConstructor::Bare(NewChangelogEntry::renamed),
    };

    /// A gated entry of `kinds` built by `constructor`, for the tests that hand the gate a
    /// misbuilt one.
    #[cfg(test)]
    pub(in crate::den) fn for_test(
        kinds: &'static [ChangelogEntryKind],
        constructor: EntryConstructor<C>,
    ) -> Self {
        Self { kinds, constructor }
    }

    /// Whether its entry may carry `kind`.
    pub(in crate::den) fn builds(&self, kind: ChangelogEntryKind) -> bool {
        self.kinds.contains(&kind)
    }

    /// Its entry on `commission`, by `actor`, at `at`, as its domain constructor builds it from
    /// `change`, the change the act recorded: `Ok(None)` only when an optional constructor
    /// finds nothing to record, and [`Unbuildable`] when its constructor takes a change and
    /// none was recorded, or takes an actor and the act has none. The system's constructor
    /// takes no actor.
    pub(in crate::den) fn construct(
        &self,
        commission: CommissionId,
        actor: Option<&UserId>,
        change: Option<&C>,
        at: DateTimeUtc,
    ) -> Result<Option<NewChangelogEntry>, Unbuildable> {
        match self.constructor {
            EntryConstructor::Bare(constructor) => {
                let actor = actor.ok_or(Unbuildable)?;
                Ok(Some(constructor(commission, actor.clone(), at)))
            }
            EntryConstructor::Change(constructor) => {
                let actor = actor.ok_or(Unbuildable)?;
                let change = change.ok_or(Unbuildable)?;
                Ok(Some(constructor(commission, actor.clone(), change, at)))
            }
            EntryConstructor::Optional(constructor) => {
                let actor = actor.ok_or(Unbuildable)?;
                let change = change.ok_or(Unbuildable)?;
                Ok(constructor(commission, actor.clone(), change, at))
            }
            EntryConstructor::System(constructor) => {
                let change = change.ok_or(Unbuildable)?;
                Ok(Some(constructor(commission, change, at)))
            }
        }
    }
}

/// Why a gated entry built nothing it had to: its constructor takes a change or an actor the
/// act doesn't have.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("an entry's constructor takes a change or an actor the act doesn't have")]
pub(in crate::den) struct Unbuildable;

impl GatedEntry<NewCommission> {
    /// The rating a create was given: kind `MaturityRated`, by
    /// `NewChangelogEntry::rated_at_create`, from what the create chose; none when it chose no
    /// rating.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const RATED_AT_CREATE: Self = Self {
        kinds: &[ChangelogEntryKind::MaturityRated],
        constructor: EntryConstructor::Optional(NewChangelogEntry::rated_at_create),
    };
}

impl GatedEntry<LifecycleMove> {
    /// A lifecycle move's entry: kind `LifecycleMoved`, by `NewChangelogEntry::lifecycle_moved`,
    /// from the move the view wrote.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const LIFECYCLE_MOVED: Self = Self {
        kinds: &[ChangelogEntryKind::LifecycleMoved],
        constructor: EntryConstructor::Change(NewChangelogEntry::lifecycle_moved),
    };
}

impl GatedEntry<Dispute> {
    /// A dispute's raise entry: kind `LifecycleMoved`, by `NewChangelogEntry::disputed`, from
    /// the dispute the view wrote.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const DISPUTED: Self = Self {
        kinds: &[ChangelogEntryKind::LifecycleMoved],
        constructor: EntryConstructor::Change(NewChangelogEntry::disputed),
    };
}

impl GatedEntry<DisputeClear> {
    /// A dispute's clear entry: kind `LifecycleMoved`, by `NewChangelogEntry::dispute_cleared`,
    /// from the clear the view wrote.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const DISPUTE_CLEARED: Self = Self {
        kinds: &[ChangelogEntryKind::LifecycleMoved],
        constructor: EntryConstructor::Change(NewChangelogEntry::dispute_cleared),
    };
}

impl GatedEntry<DirectionSet> {
    /// A direction status set's entry: kind `StatusChanged`, by
    /// `NewChangelogEntry::direction_set`, from the set the view wrote.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const STATUS_SET: Self = Self {
        kinds: &[ChangelogEntryKind::StatusChanged],
        constructor: EntryConstructor::Change(NewChangelogEntry::direction_set),
    };
}

impl GatedEntry<DirectionCleared> {
    /// A direction status clear's entry: kind `StatusChanged`, by
    /// `NewChangelogEntry::direction_cleared`, from the clear the view wrote.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const STATUS_CLEARED: Self = Self {
        kinds: &[ChangelogEntryKind::StatusChanged],
        constructor: EntryConstructor::Change(NewChangelogEntry::direction_cleared),
    };
}

impl GatedEntry<DeadlineSet> {
    /// A deadline set's entry: kind `DeadlineSet`, or `DeadlineExtended` for a later deadline,
    /// by `NewChangelogEntry::deadline_set`, from the set the view wrote.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const DEADLINE_SET: Self = Self {
        kinds: &[
            ChangelogEntryKind::DeadlineSet,
            ChangelogEntryKind::DeadlineExtended,
        ],
        constructor: EntryConstructor::Change(NewChangelogEntry::deadline_set),
    };
}

impl GatedEntry<DeadlineCleared> {
    /// A deadline clear's entry: kind `DeadlineSet`, by `NewChangelogEntry::deadline_cleared`,
    /// from the clear the view wrote.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const DEADLINE_CLEARED: Self = Self {
        kinds: &[ChangelogEntryKind::DeadlineSet],
        constructor: EntryConstructor::Change(NewChangelogEntry::deadline_cleared),
    };
}

impl GatedEntry<DelayedSet> {
    /// A Delayed flag's entry: kind `Delayed`, by `NewChangelogEntry::delayed_set`, from the
    /// set the view wrote.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const DELAYED_SET: Self = Self {
        kinds: &[ChangelogEntryKind::Delayed],
        constructor: EntryConstructor::Change(NewChangelogEntry::delayed_set),
    };
}

impl GatedEntry<DelayedCleared> {
    /// A Delayed flag taken down's entry: kind `Delayed`, by
    /// `NewChangelogEntry::delayed_cleared`, from the clear the view wrote.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const DELAYED_CLEARED: Self = Self {
        kinds: &[ChangelogEntryKind::Delayed],
        constructor: EntryConstructor::Change(NewChangelogEntry::delayed_cleared),
    };
}

impl GatedEntry<MaturitySet> {
    /// A rating's entry: kind `MaturityRated`, by `NewChangelogEntry::maturity_rated`, from the
    /// rating the view wrote.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const MATURITY_RATED: Self = Self {
        kinds: &[ChangelogEntryKind::MaturityRated],
        constructor: EntryConstructor::Change(NewChangelogEntry::maturity_rated),
    };
}

impl GatedEntry<SlotDeclaration> {
    /// A Slot declare's entry: kind `SlotDeclared`, by `NewChangelogEntry::slot_declared`,
    /// from the declaration the view wrote.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const SLOT_DECLARED: Self = Self {
        kinds: &[ChangelogEntryKind::SlotDeclared],
        constructor: EntryConstructor::Change(NewChangelogEntry::slot_declared),
    };
}

impl GatedEntry<LateMark> {
    /// The system's Late entry: kind `Late`, by `NewChangelogEntry::late`, by no actor, from
    /// the mark the act's rule checked.
    #[expect(
        clippy::disallowed_methods,
        reason = "a gated entry names its one domain constructor here, and only here"
    )]
    pub(crate) const LATE: Self = Self {
        kinds: &[ChangelogEntryKind::Late],
        constructor: EntryConstructor::System(NewChangelogEntry::late),
    };
}

/// Which gated entries the gate writes once an act lands: one on every landing, one only when
/// the act changed its node or its view's write landed, or, for the create alone, one on every
/// landing and then a second when its constructor finds something to record.
#[derive(Debug)]
pub(crate) enum EntryRule<C> {
    /// One entry, whatever the act changed.
    Always(GatedEntry<C>),
    /// One entry when the act changed something; none when it changed nothing.
    OnChange(GatedEntry<C>),
    /// The first entry whatever the act changed, then the second right after it, when its
    /// constructor finds something to record in the act's change.
    AlwaysThen(GatedEntry<C>, GatedEntry<C>),
}

/// The gated entries an act that landed writes, in order: the one it must build, then one it
/// builds only when its constructor finds something to record.
#[derive(Debug)]
pub(crate) struct Planned<C> {
    /// The entry the act must build.
    pub(crate) first: GatedEntry<C>,
    /// The entry that follows it, when its constructor builds one.
    pub(crate) then: Option<GatedEntry<C>>,
}

impl<C> EntryRule<C> {
    /// The gated entries an act that `changed` something writes, or `None`.
    pub(crate) fn planned(self, changed: bool) -> Option<Planned<C>> {
        match self {
            Self::Always(first) => Some(Planned { first, then: None }),
            Self::OnChange(first) => changed.then_some(Planned { first, then: None }),
            Self::AlwaysThen(first, then) => Some(Planned {
                first,
                then: Some(then),
            }),
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

/// Why a handler refused its act on what its lent view read, before its write: a typed
/// refusal, never a store failure. It carries nothing of the act.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HandlerRefusal {
    /// The tab the act names is none of the commission's: missing and foreign alike.
    UnknownTab,
    /// The tab declares no such surface.
    UnknownSurface,
}

/// Why a type's handler failed: a store failure under it, or its typed refusal.
#[derive(Debug, thiserror::Error)]
pub(crate) enum HandlerError {
    /// A store under the handler failed.
    #[error("a store under the handler failed")]
    Store(#[from] anyhow::Error),
    /// The handler refused its act on what its view read.
    #[error("the handler refused the act")]
    Refused(HandlerRefusal),
}

#[cfg(test)]
mod tests;
