//! The Den's write: one use case every typed write hands its act to. It asks the pool's
//! verdict, opens the unit, makes the grant from the locked standing, builds Atta's act from
//! the grant alone, and has Atta check it and the write host apply it.

use std::fmt;

use atta::{Act, ActName, Born, Placed, RealPath, Shape, WriteCaps, WriteError, Writer};
use domain::{
    datetime::DateTimeUtc,
    elements::{level::Level, user::UserId},
    ports::Unit,
};

use crate::{
    LazyUnit, Ports,
    commission::den::CommissionPlace,
    den::{
        Den, DenName, DenThing, DenType, Place, TopLevel,
        act::{ActShape, ActSpec, ArmAnswer, BornEntry, DenAct},
        authority::{self, Grant},
        gate::{DenApplied, DenRefusal, DenWriteHost},
        host::{DenHost, type_key},
        level::{atta_level, den_level},
        names,
    },
    use_case,
};

/// A write: who acts, and the act.
pub(crate) struct Command {
    /// The session's user.
    pub(crate) actor: UserId,
    /// What they do.
    pub(crate) act: DenAct,
}

/// What a write answers, from what the gate wrote: the thing it acted on, the name it gave,
/// the level of the node it acted on or made, and what the type's handler answered.
pub(crate) struct WriteResult {
    /// The thing written, a new one's id included.
    pub(crate) thing: DenThing,
    /// The name a create or a rename wrote; `None` for a change.
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
    /// rename or change, or a role below the capability's.
    WrongGrant,
    /// A made node would be wider than Private, which nothing may be until 0.6.
    WiderThanPrivate,
    /// Another node already holds the made node's key.
    SegmentTaken,
    /// The type's handler refused the act.
    TypeRefused,
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

impl From<DenRefusal> for RefusalClass {
    /// The gate's refusal under the den's word for it.
    fn from(refusal: DenRefusal) -> Self {
        match refusal {
            DenRefusal::WrongGrant => Self::WrongGrant,
            DenRefusal::WiderThanPrivate => Self::WiderThanPrivate,
            DenRefusal::SegmentTaken => Self::SegmentTaken,
            DenRefusal::TypeRefused => Self::TypeRefused,
        }
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
    /// Atta, the gate or the type refused the act.
    #[error("the den refused the act")]
    Refused(RefusalClass),
    /// A store behind the Den failed, or the den broke one of its own rules.
    #[error("the den could not be written")]
    Store(#[from] anyhow::Error),
}

/// A store failure, or a rule of the den's own broken, as the write's error.
fn broken(what: &'static str) -> DenWriteError {
    DenWriteError::Store(anyhow::anyhow!(what))
}

/// The real path of `/commission`, the top-level directory a create makes a commission in.
pub(in crate::den) fn commission_directory() -> Option<RealPath> {
    let segment = names::fixed_segment(TopLevel::Commission.into())?;
    Some(RealPath::from(vec![segment]))
}

/// The existing node the grant names, as Atta's act carries it: its path from the grant's
/// commission id, its type the act's, its level the locked read's.
fn granted_node(grant: &Grant, den_type: DenType) -> Result<Placed, DenWriteError> {
    let authority::GrantKind::Existing {
        commission, level, ..
    } = grant.kind()
    else {
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

/// The den's one builder of Atta's act: every path from the grant's typed ids, an existing
/// node's level from the locked read, a create's key from the id the grant minted.
fn build(spec: ActSpec, body: DenAct, grant: &Grant) -> Result<Act<DenAct>, DenWriteError> {
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
            let minted = grant
                .minted()
                .ok_or(DenWriteError::Refused(RefusalClass::WrongGrant))?;
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
        ActShape::Rename { name } => Shape::Rename {
            node: granted_node(grant, den_type)?,
            name: names::display_name(&name),
        },
        ActShape::Change => Shape::Change {
            node: granted_node(grant, den_type)?,
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

/// Steps four and five on an open unit: build Atta's act from `grant`, then have Atta check it
/// and the write host apply it. A refusal of any kind is an `Err`, so the unit rolls back.
pub(in crate::den) async fn written(
    unit: Unit<'_>,
    grant: Grant,
    act: DenAct,
    now: DateTimeUtc,
) -> Result<WriteResult, DenWriteError> {
    let spec = act.spec();
    let atta_act = build(spec, act, &grant)?;
    let writer = Writer {
        viewer: DenHost::viewer_of(grant.actor()),
        caps: WriteCaps::default(),
    };
    let mut host = DenWriteHost::new(unit, grant, now);
    let applied = writer
        .act(&mut host, atta_act)
        .await
        .map_err(write_error)?
        .map_err(|refusal| DenWriteError::Refused(RefusalClass::from(refusal)))?;
    answer(applied)
}

impl Den<'_> {
    /// Writes the Den: the pool's verdict first, refusing with no unit opened; then, in the
    /// unit, the grant from the locked standing, Atta's checks and the write host's apply.
    #[use_case]
    pub(crate) async fn write(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        command: Command,
        now: DateTimeUtc,
    ) -> Result<WriteResult, DenWriteError> {
        let Command { actor, act } = command;
        let spec = act.spec();
        authority::admit(ports, &spec, &actor).await?;

        let unit = uow.open().await?;
        let grant = authority::grant(&mut *unit, &spec, &actor).await?;
        written(unit, grant, act, now).await
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod log_tests;
