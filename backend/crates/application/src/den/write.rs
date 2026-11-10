//! The Den's write: one use case every typed write hands its acts to, one or several on one
//! thing. It asks the pool's verdict, opens the unit, makes the grant from the locked standing,
//! builds each act's Atta act from the grant alone, and has Atta check each and the write host
//! apply it, all in the one unit.

use std::fmt;

use atta::{Act, ActName, Born, Placed, RealPath, Shape, Viewer, WriteCaps, WriteError, Writer};
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, CommissionRefusal},
        level::Level,
        user::UserId,
    },
    ports::{New, Unit},
};

use crate::{
    LazyUnit, Ports,
    commission::{den::CommissionPlace, sweep::SweepToken},
    den::{
        Den, DenName, DenThing, DenType, Place, TopLevel,
        act::{ActShape, ActSpec, ArmAnswer, BornEntry, DenAct, HandlerRefusal},
        authority::{self, Grant},
        gate::{DenApplied, DenRefusal, DenWriteHost},
        host::{DenHost, type_key},
        level::{atta_level, den_level},
        names,
    },
    use_case,
};

/// Atta's viewer for the system: no DID can equal it, since a DID starts `did:` and holds no
/// angle bracket.
const SYSTEM_VIEWER: &str = "<system>";

/// Who acts in a write: the session's user, or the system, which only the deadline sweep can
/// name, since only it can make a [`SweepToken`].
pub(crate) enum Actor {
    /// The session's user.
    User(UserId),
    /// The system, with no user behind it.
    System(SweepToken),
}

impl Actor {
    /// The user acting, or `None` for the system.
    pub(in crate::den) fn user(&self) -> Option<&UserId> {
        match self {
            Self::User(user) => Some(user),
            Self::System(_) => None,
        }
    }

    /// Atta's viewer for this actor: a user's from their DID, the system's a fixed token.
    pub(in crate::den) fn viewer(&self) -> Viewer {
        match self {
            Self::User(user) => DenHost::viewer_of(user),
            Self::System(_) => Viewer::from(SYSTEM_VIEWER.to_owned()),
        }
    }
}

impl From<UserId> for Actor {
    /// The session's user, acting.
    fn from(user: UserId) -> Self {
        Self::User(user)
    }
}

impl fmt::Debug for Actor {
    /// Which actor it is; never the user's DID.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::User(_) => formatter.write_str("User"),
            Self::System(_) => formatter.write_str("System"),
        }
    }
}

/// A write: who acts, and their acts, each anchored on the same thing and needing the same,
/// all written in one unit.
pub(crate) struct Command {
    /// The session's user, or the system.
    pub(crate) actor: Actor,
    /// What they do, in order: one act, or several on one thing.
    pub(crate) acts: Vec<DenAct>,
}

/// What a write answers, from what the gate wrote: the thing it acted on, the name it gave,
/// the level of the node it acted on or made, and what the type's handler answered.
pub(crate) struct WriteResult {
    /// The thing written, a new one's id included.
    pub(crate) thing: DenThing,
    /// The name a create or a rename left on the node.
    pub(crate) name: Option<DenName>,
    /// A made node's settled level, or the level of the node acted on.
    pub(crate) level: Level,
    /// The thing the handler acted on, as its lent view read it.
    pub(crate) answer: ArmAnswer,
}

impl fmt::Debug for WriteResult {
    /// The level and the answer's id; never the name, nor a DID.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WriteResult")
            .field("level", &self.level)
            .field("answer", &self.answer)
            .finish_non_exhaustive()
    }
}

/// Why a write was refused, by class: Atta's own refusals and the den's. Its word is for logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum RefusalClass {
    /// A node would be made inside a file, or a made file would hold entries.
    EntriesInFile,
    /// A made node would be wider than its parent, or an entry than its node.
    WiderThanParent,
    /// A made node would lie deeper than the cap.
    TooDeep,
    /// The act would act on, or make, a node of another type than its own.
    OtherType,
    /// Two entries of one act would share a key.
    DuplicateKey,
    /// The grant doesn't fit the act: an existing thing's for a create, a create's for a
    /// rename, or a role below the capability's.
    WrongGrant,
    /// A made node would be wider than Private, which nothing may be until 0.6.
    WiderThanPrivate,
    /// Another node already holds the made node's key.
    SegmentTaken,
}

/// Atta's refusal under the den's word for it. A function, not `From`: the Atta-exposure
/// guard counts every trait impl that names Atta as public, whatever its type's visibility.
fn refusal_class(refusal: atta::Refusal) -> RefusalClass {
    match refusal {
        atta::Refusal::EntriesInFile => RefusalClass::EntriesInFile,
        atta::Refusal::WiderThanParent => RefusalClass::WiderThanParent,
        atta::Refusal::TooDeep => RefusalClass::TooDeep,
        atta::Refusal::OtherType => RefusalClass::OtherType,
        atta::Refusal::DuplicateKey => RefusalClass::DuplicateKey,
    }
}

/// The gate's refusal as the write's error: an act's own rule's refusal as itself, the gate's
/// own by the den's word for it. A function, not `From`: one refusal maps to two variants.
fn gate_refusal(refusal: DenRefusal) -> DenWriteError {
    match refusal {
        DenRefusal::WrongGrant => DenWriteError::Refused(RefusalClass::WrongGrant),
        DenRefusal::WiderThanPrivate => DenWriteError::Refused(RefusalClass::WiderThanPrivate),
        DenRefusal::SegmentTaken => DenWriteError::Refused(RefusalClass::SegmentTaken),
        DenRefusal::Precondition(refusal) => DenWriteError::Precondition(refusal),
        DenRefusal::Handler(refusal) => DenWriteError::Handler(refusal),
    }
}

/// Why a Den write failed. `Display` is fixed text and never interpolates a cause; a store's
/// cause rides `source()`.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DenWriteError {
    /// No such thing, or the actor holds no standing on it: the same answer for both.
    #[error("node not found")]
    NotFound,
    /// The actor's role is below what the act needs.
    #[error("the act needs a higher role")]
    Forbidden,
    /// Atta or the gate refused the act.
    #[error("the den refused the act")]
    Refused(RefusalClass),
    /// The act's own domain rule refused it, on the facts its grant read.
    #[error("the act's rule refused it")]
    Precondition(CommissionRefusal),
    /// The act's handler refused it on what its lent view read, before its write.
    #[error("the act's handler refused it")]
    Handler(HandlerRefusal),
    /// A store behind the Den failed, or the den broke one of its own rules.
    #[error("the den could not be written")]
    Store(#[from] anyhow::Error),
}

/// A store failure, or a rule of the den's own broken, as the write's error.
pub(in crate::den) fn broken(what: &'static str) -> DenWriteError {
    DenWriteError::Store(anyhow::anyhow!(what))
}

/// The real path of `/commission`, the top-level directory a create makes a commission in.
pub(in crate::den) fn commission_directory() -> Option<RealPath> {
    let segment = names::fixed_segment(TopLevel::Commission.into())?;
    Some(RealPath::from(vec![segment]))
}

/// What the den's builder anchors an act on: an existing commission at its own level, or a
/// new one under the id a create's grant minted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Target {
    /// An existing commission, at its own level.
    Existing {
        /// The commission.
        commission: CommissionId,
        /// Its own level.
        level: Level,
    },
    /// A commission not made yet, under the id the den minted.
    Minted(New<CommissionId>),
}

impl From<&Grant> for Target {
    /// What the grant leaves its actor to write: its commission at the level the locked read
    /// answered, or the id it minted.
    fn from(grant: &Grant) -> Self {
        match grant.kind() {
            authority::GrantKind::Existing {
                commission, level, ..
            }
            | authority::GrantKind::System {
                commission, level, ..
            } => Self::Existing {
                commission: *commission,
                level: *level,
            },
            authority::GrantKind::Create { commission } => Self::Minted(*commission),
        }
    }
}

/// The existing node `target` names, as Atta's act carries it: its path from the commission's
/// id, its type the act's, its level the target's.
fn targeted_node(target: &Target, den_type: DenType) -> Result<Placed, DenWriteError> {
    let Target::Existing { commission, level } = target else {
        return Err(DenWriteError::Refused(RefusalClass::WrongGrant));
    };
    let place = Place::Commission(CommissionPlace::Commission(*commission));
    let path = place
        .anchored_path()
        .ok_or_else(|| broken("a commission id makes no path"))?;
    let placed = Placed {
        path,
        typ: type_key(den_type),
        level: atta_level(*level),
    };
    Ok(placed)
}

/// An entry a create makes, as Atta's act carries it: its word its key and its name.
fn born_entry(entry: BornEntry) -> Result<Born, DenWriteError> {
    let key = names::fixed_segment(entry.word).ok_or_else(|| broken("a fixed word is no key"))?;
    let name = names::fixed_name(entry.word).ok_or_else(|| broken("a fixed word is no name"))?;
    let born = Born {
        key,
        typ: type_key(entry.den_type),
        name: names::display_name(&name),
        level: entry.level.map(atta_level),
    };
    Ok(born)
}

/// The den's one builder of Atta's act: every path from the target's typed ids, an existing
/// node's level from the target, a create's key from the id the grant minted; a create in an
/// entry is made under the targeted commission's entry, which it takes as Private, keyed by its
/// own id; a field act is a change on the targeted commission's node.
fn build(spec: ActSpec, body: DenAct, target: &Target) -> Result<Act<DenAct>, DenWriteError> {
    let ActSpec {
        word,
        den_type,
        shape,
        ..
    } = spec;
    let shape = match shape {
        ActShape::Create {
            name,
            level,
            entries,
        } => {
            let Target::Minted(minted) = target else {
                return Err(DenWriteError::Refused(RefusalClass::WrongGrant));
            };
            let path =
                commission_directory().ok_or_else(|| broken("`commission` is no segment"))?;
            let parent = Placed {
                path,
                typ: type_key(DenType::TopLevel),
                level: atta::Level::Public,
            };
            let key = names::id_segment(minted.as_ref())
                .ok_or_else(|| broken("a minted id is no key"))?;
            let node = Born {
                key,
                typ: type_key(den_type),
                name: names::display_name(&name),
                level: level.map(atta_level),
            };
            let entries = entries
                .into_iter()
                .map(born_entry)
                .collect::<Result<Vec<Born>, DenWriteError>>()?;
            Shape::Create {
                parent,
                node,
                entries,
            }
        }
        ActShape::CreateInEntry {
            entry,
            entry_type,
            key,
            name,
        } => {
            let Target::Existing { commission, .. } = target else {
                return Err(DenWriteError::Refused(RefusalClass::WrongGrant));
            };
            let place = Place::Commission(CommissionPlace::Commission(*commission));
            let entry_segment =
                names::fixed_segment(entry).ok_or_else(|| broken("a fixed word is no key"))?;
            let path = place
                .anchored_path()
                .ok_or_else(|| broken("a commission id makes no path"))?
                .join(entry_segment);
            let parent = Placed {
                path,
                typ: type_key(entry_type),
                level: atta::Level::Private,
            };
            let key = names::id_segment(key).ok_or_else(|| broken("an id is no key"))?;
            let node = Born {
                key,
                typ: type_key(den_type),
                name: names::display_name(&name),
                level: None,
            };
            Shape::Create {
                parent,
                node,
                entries: Vec::new(),
            }
        }
        ActShape::Rename { name } => Shape::Rename {
            node: targeted_node(target, den_type)?,
            name: names::display_name(&name),
        },
        ActShape::Change => Shape::Change {
            node: targeted_node(target, den_type)?,
        },
    };
    let act = Act {
        name: ActName::from(word.to_owned()),
        typ: type_key(den_type),
        shape,
        body,
    };
    Ok(act)
}

/// Atta's write error as the den's: a refusal by its class, a host failure a store failure.
fn write_error(error: WriteError) -> DenWriteError {
    match error {
        WriteError::Refused(refusal) => DenWriteError::Refused(refusal_class(refusal)),
        WriteError::Host(host_error) => DenWriteError::Store(anyhow::Error::from(host_error)),
    }
}

/// What the gate wrote, as the write's answer; the name is re-checked by the Den's rule.
fn answer(applied: DenApplied) -> Result<WriteResult, DenWriteError> {
    let name = applied
        .name
        .map(|name| DenName::try_from(name.as_ref()))
        .transpose()
        .map_err(|refusal| {
            let refused_name =
                anyhow::Error::new(refusal).context("the den wrote a name the Den's rule refuses");
            DenWriteError::Store(refused_name)
        })?;
    let written = WriteResult {
        thing: DenThing::Commission(applied.commission),
        name,
        level: den_level(applied.level),
        answer: applied.answer,
    };
    Ok(written)
}

/// Atta's writer for `viewer`, with the den's caps: the one the write acts through and the one
/// the acts list dry-runs, so the two meet the same checks.
fn den_writer(viewer: Viewer) -> Writer {
    Writer {
        viewer,
        caps: WriteCaps::default(),
    }
}

/// Steps four and five on an open unit: build Atta's act of each of `acts` from `grant`, then
/// have Atta check each and the write host apply it, in order, over one host and one unit. A
/// refusal of any kind is an `Err`, so the unit rolls back whatever the earlier acts wrote.
/// Answers the last act's result, which reads the thing after every act.
pub(in crate::den) async fn written(
    unit: Unit<'_>,
    grant: Grant,
    acts: Vec<DenAct>,
    now: DateTimeUtc,
) -> Result<WriteResult, DenWriteError> {
    let target = Target::from(&grant);
    let atta_acts = acts
        .into_iter()
        .map(|act| build(act.spec(), act, &target))
        .collect::<Result<Vec<Act<DenAct>>, DenWriteError>>()?;
    let writer = den_writer(grant.actor().viewer());
    let mut host = DenWriteHost::new(unit, grant, now);
    let mut last = None;
    for atta_act in atta_acts {
        let applied = writer
            .act(&mut host, atta_act)
            .await
            .map_err(write_error)?
            .map_err(gate_refusal)?;
        last = Some(applied);
    }
    let applied = last.ok_or_else(|| broken("a write carries no act"))?;
    answer(applied)
}

/// The spec every one of `acts` shares: the first act's, when each is anchored where it is and
/// needs what it needs. No act, or acts anchored apart, is a broken rule.
fn shared_spec(acts: &[DenAct]) -> Result<ActSpec, DenWriteError> {
    let (first, rest) = acts
        .split_first()
        .ok_or_else(|| broken("a write carries no act"))?;
    let spec = first.spec();
    let apart = rest
        .iter()
        .map(DenAct::spec)
        .any(|other| other.anchor != spec.anchor || other.needs != spec.needs);
    if apart {
        return Err(broken("a write's acts are anchored apart"));
    }
    Ok(spec)
}

/// Whether Atta's checks pass the act the den's builder makes of `act` on the existing
/// `commission` at `level`, for `viewer`, as a write would meet them; nothing is applied and no
/// unit is opened. Atta never reads an act's body, so the checks run on the act without it.
pub(in crate::den) fn atta_passes(
    ports: &Ports,
    viewer: &UserId,
    commission: CommissionId,
    level: Level,
    act: DenAct,
) -> Result<bool, DenWriteError> {
    let target = Target::Existing { commission, level };
    let spec = act.spec();
    let Act {
        name, typ, shape, ..
    } = build(spec, act, &target)?;
    let bodiless = Act {
        name,
        typ,
        shape,
        body: (),
    };
    let host = DenHost::for_check(ports, viewer);
    let writer = den_writer(DenHost::viewer_of(viewer));
    let passes = writer.dry_run(&host, &bodiless).is_ok();
    Ok(passes)
}

impl Den<'_> {
    /// Writes the Den: the pool's verdict first, refusing with no unit opened; then, in the
    /// unit, the grant from the locked standing, and Atta's checks and the write host's apply
    /// for each act in turn.
    #[use_case]
    pub(crate) async fn write(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        command: Command,
        now: DateTimeUtc,
    ) -> Result<WriteResult, DenWriteError> {
        let Command { actor, acts } = command;
        let spec = shared_spec(&acts)?;
        authority::admit(ports, &spec, &actor).await?;

        let unit = uow.open().await?;
        let grant = authority::grant(&mut *unit, &spec, actor).await?;
        written(unit, grant, acts, now).await
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod log_tests;
