//! The den's write host and its gate: the one place a write reaches the node rows, the one
//! issuer of a write capability, and the one appender of a write's changelog entry.
//!
//! The gate reads the node rows it writes outside Atta's resolver, to tie the write to them;
//! it serves nothing it reads. Create and rename answer from what the gate wrote, never from
//! a second read of the node.

use std::sync::{Mutex, PoisonError};

use atta::{
    Access, Admission, Born, Checked, DisplayName, Found, Host, HostError, Kind, Metadata, Mount,
    Placed, Reach, RealPath, Segment, Shape, Type, Viewer,
};
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{AsOwner, AsParticipant, CommissionId, CommissionRole, NewChangelogEntry},
        node::{NewNode, NodeId, NodeRow, NodeSegment, NodeType},
        text::StoredText,
    },
    ports::{Creator, New, NodeRepo, SegmentTaken, Unit, WriteCapability},
};

use crate::den::{
    act::{DenAct, EntryDraft, HandlerError},
    authority::Grant,
    host::{DenHost, host_error, type_kind},
    level::den_level,
    names,
    write::commission_directory,
};

/// Why the gate refused an act it was handed; it carries nothing of the act.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
pub(in crate::den) enum DenRefusal {
    /// The grant doesn't fit the act, or its role is below the capability's.
    WrongGrant,
    /// A made node would be wider than Private.
    WiderThanPrivate,
    /// Another node already holds the made node's key.
    SegmentTaken,
    /// The type's handler refused the act.
    TypeRefused,
}

/// What the gate wrote: the commission written, the name Atta checked, and a made node's
/// settled level or the acted-on node's level.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
pub(in crate::den) struct DenApplied {
    /// The commission written.
    pub(in crate::den) commission: CommissionId,
    /// The name a create or a rename wrote.
    pub(in crate::den) name: Option<DisplayName>,
    /// A made node's settled level, or the level of the node acted on.
    pub(in crate::den) level: atta::Level,
}

/// What a gate step answers: a store failure as `Err`, the gate's refusal inside it.
type Gated<T> = anyhow::Result<Result<T, DenRefusal>>;

/// The host Atta hands a checked act to, for one write: it serves no read and applies acts
/// only through the gate. Built once, by the den's write.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
pub(in crate::den) struct DenWriteHost<'u> {
    viewer: Viewer,
    unit: Mutex<Unit<'u>>,
    grant: Grant,
    now: DateTimeUtc,
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
impl<'u> DenWriteHost<'u> {
    /// A host over `unit` for the grant's actor, applying acts under `grant` at `now`. The
    /// unit sits in a `Mutex` only because the engine's host must be `Sync`.
    pub(in crate::den) fn new(unit: Unit<'u>, grant: Grant, now: DateTimeUtc) -> Self {
        Self {
            viewer: DenHost::viewer_of(grant.actor()),
            unit: Mutex::new(unit),
            grant,
            now,
        }
    }
}

/// The host's refusal of an act Atta passed for another viewer: a broken rule, never a miss.
#[derive(Debug, thiserror::Error)]
#[error("the write host applies acts for its own viewer only")]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
struct AnotherViewer;

#[async_trait::async_trait]
impl Host for DenWriteHost<'_> {
    type Act = DenAct;
    type Applied = Result<DenApplied, DenRefusal>;

    /// Nothing: the write host serves no read.
    async fn child(
        &self,
        _viewer: &Viewer,
        _parent: &RealPath,
        _key: &Segment,
    ) -> Result<Option<Found>, HostError> {
        Ok(None)
    }

    /// Nothing: the write host serves no read.
    async fn children(
        &self,
        _viewer: &Viewer,
        _parent: &RealPath,
    ) -> Result<Vec<Found>, HostError> {
        Ok(Vec::new())
    }

    /// Nothing: the write host serves no read.
    async fn mounts(&self, _viewer: &Viewer, _folder: &RealPath) -> Result<Vec<Mount>, HostError> {
        Ok(Vec::new())
    }

    /// As every den host answers it.
    fn kind(&self, typ: &Type) -> Kind {
        type_kind(typ)
    }

    /// Refuses: the write host serves no read.
    fn admit(
        &self,
        _viewer: &Viewer,
        _path: &RealPath,
        _node: &Metadata,
        _access: Access,
        _reach: Reach,
    ) -> Admission {
        Admission::Refuse
    }

    /// Runs the gate on the act, for this host's own viewer only.
    async fn apply(
        &mut self,
        viewer: &Viewer,
        checked: Checked<DenAct>,
    ) -> Result<Self::Applied, HostError> {
        if viewer != &self.viewer {
            return Err(host_error(anyhow::Error::new(AnotherViewer)));
        }
        let unit = self.unit.get_mut().unwrap_or_else(PoisonError::into_inner);
        gate(&mut **unit, &self.grant, checked, self.now)
            .await
            .map_err(host_error)
    }
}

/// Which grant a capability's role needs, and the commission its thing names: the gate's role
/// table. A den trait because the markers are workflow's and kernel's, with no trait of their
/// own, and only this gate may read it.
pub(in crate::den) trait RoleTable: Sized {
    /// What the capability is over.
    type Thing;

    /// The thing `grant` leaves this role to write, or `None` when it doesn't.
    fn granted(grant: &Grant) -> Option<Self::Thing>;

    /// The commission `thing` names.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "only the test probe's handler is lent a capability until the next slice"
        )
    )]
    fn commission(thing: &Self::Thing) -> CommissionId;
}

impl RoleTable for AsParticipant {
    type Thing = CommissionId;

    /// Any Participant, the Owner included.
    fn granted(grant: &Grant) -> Option<CommissionId> {
        grant.existing_at_least(CommissionRole::Participant)
    }

    fn commission(thing: &CommissionId) -> CommissionId {
        *thing
    }
}

impl RoleTable for AsOwner {
    type Thing = CommissionId;

    /// The Owner only.
    fn granted(grant: &Grant) -> Option<CommissionId> {
        grant.existing_at_least(CommissionRole::Owner)
    }

    fn commission(thing: &CommissionId) -> CommissionId {
        *thing
    }
}

impl RoleTable for Creator {
    type Thing = New<CommissionId>;

    /// A create grant only.
    fn granted(grant: &Grant) -> Option<New<CommissionId>> {
        grant.minted()
    }

    fn commission(thing: &New<CommissionId>) -> CommissionId {
        *thing.as_ref()
    }
}

/// The write capability in role `R` that `grant` leaves, or `None`; the one issuer in the
/// workspace.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
pub(in crate::den) fn issue<R: RoleTable>(grant: &Grant) -> Option<WriteCapability<R::Thing, R>> {
    let thing = R::granted(grant)?;
    #[expect(
        clippy::disallowed_methods,
        reason = "the den's write gate is the one issuer of a write capability"
    )]
    let capability = WriteCapability::gate_issued(thing);
    Some(capability)
}

/// Whether `error`, from the node port, says another node already holds the segment.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
fn segment_taken(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| cause.is::<SegmentTaken>())
}

/// A failure of the den's own rules: the facts the act was built from don't match the rows.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
fn facts_lied(what: &'static str) -> anyhow::Error {
    anyhow::anyhow!(what)
}

/// The node type a den type is stored as; a type the node rows don't hold is a broken rule.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
fn node_type(typ: &Type) -> anyhow::Result<NodeType> {
    typ.as_ref()
        .parse::<NodeType>()
        .map_err(|_| facts_lied("the act's type is no node type"))
}

/// A segment as the node rows hold it.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
fn node_segment(segment: &Segment) -> anyhow::Result<NodeSegment> {
    NodeSegment::try_from(segment.as_ref()).map_err(anyhow::Error::new)
}

/// The row of the existing node the act acts on, walked in the unit from the grant's
/// commission: `/commission/<the grant's id>`, then each deeper step of `placed`'s path. A
/// row missing, or of another type than `placed`'s, means the facts lied.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
async fn walk(rows: &mut dyn NodeRepo, grant: &Grant, placed: &Placed) -> anyhow::Result<NodeRow> {
    let commission = grant
        .existing_at_least(CommissionRole::Participant)
        .ok_or_else(|| facts_lied("an existing node's act holds no existing grant"))?;
    let directory =
        commission_directory().ok_or_else(|| facts_lied("`commission` is no segment"))?;
    let segments = placed.path.segments();
    let (top, deeper) = segments
        .split_at_checked(2)
        .ok_or_else(|| facts_lied("an existing node's path is shallower than a commission"))?;
    if top.first() != directory.segments().first() {
        return Err(facts_lied(
            "an existing node's path is not under `/commission`",
        ));
    }
    let key = names::id_segment(commission).ok_or_else(|| facts_lied("an id is no key"))?;
    let mut row = rows
        .at_top(&NodeType::Commission, &node_segment(&key)?)
        .await?
        .ok_or_else(|| facts_lied("the granted commission has no node row"))?;
    for step in deeper {
        row = rows
            .child(&row.id, &node_segment(step)?)
            .await?
            .ok_or_else(|| facts_lied("a walked node has no row"))?;
    }
    if row.node_type != node_type(&placed.typ)? {
        return Err(facts_lied("a walked row's type differs from the act's"));
    }
    Ok(row)
}

/// A node row to create from a born node and the level Atta settled.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
fn new_node(parent: Option<NodeId>, born: &Born, settled: atta::Level) -> anyhow::Result<NewNode> {
    let node = NewNode {
        id: NodeId::mint(),
        parent,
        segment: node_segment(&born.key)?,
        node_type: node_type(&born.typ)?,
        level: den_level(settled),
        name: StoredText::from(born.name.as_ref().to_owned()),
    };
    Ok(node)
}

/// Creates `node` in the unit; another node holding its segment is the gate's refusal.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
async fn create(rows: &mut dyn NodeRepo, node: &NewNode) -> Gated<()> {
    match rows.create(node).await {
        Ok(()) => Ok(Ok(())),
        Err(error) if segment_taken(&error) => Ok(Err(DenRefusal::SegmentTaken)),
        Err(error) => Err(error),
    }
}

/// Writes a create's rows: its node at the top under the grant's minted id, then its entries
/// under it, each at the level Atta settled. `/commission` is a rule, not a row.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
async fn write_create(
    rows: &mut dyn NodeRepo,
    grant: &Grant,
    parent: &Placed,
    born: &[(Born, atta::Level)],
) -> Gated<DenApplied> {
    let minted = grant
        .minted()
        .ok_or_else(|| facts_lied("a create holds no create grant"))?;
    let commission = *minted.as_ref();
    if Some(&parent.path) != commission_directory().as_ref() {
        return Err(facts_lied("a create's parent is not `/commission`"));
    }
    let Some(((node, level), entries)) = born.split_first() else {
        return Err(facts_lied("a create makes no node"));
    };
    if names::id_segment(commission).as_ref() != Some(&node.key) {
        return Err(facts_lied("a create's key is not the id the grant minted"));
    }
    let made = new_node(None, node, *level)?;
    if let Err(refusal) = create(rows, &made).await? {
        return Ok(Err(refusal));
    }
    for (entry, entry_level) in entries {
        let made_entry = new_node(Some(made.id), entry, *entry_level)?;
        if let Err(refusal) = create(rows, &made_entry).await? {
            return Ok(Err(refusal));
        }
    }
    let applied = DenApplied {
        commission,
        name: Some(node.name.clone()),
        level: *level,
    };
    Ok(Ok(applied))
}

/// Writes the rows of `shape` from Atta's checked act: a create's node and entries, a
/// rename's new name on the walked row; a change writes no row yet.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
async fn write_rows(
    rows: &mut dyn NodeRepo,
    grant: &Grant,
    shape: &Shape,
    born: &[(Born, atta::Level)],
) -> Gated<DenApplied> {
    match shape {
        Shape::Create { parent, .. } => write_create(rows, grant, parent, born).await,
        Shape::Rename { node, name } => {
            let row = walk(rows, grant, node).await?;
            let stored = StoredText::from(name.as_ref().to_owned());
            if !rows.set_name(&row.id, &stored).await? {
                return Err(facts_lied("a walked row is gone"));
            }
            let applied = DenApplied {
                commission: *row_commission(grant)?,
                name: Some(name.clone()),
                level: node.level,
            };
            Ok(Ok(applied))
        }
        Shape::Change { node } => {
            walk(rows, grant, node).await?;
            let applied = DenApplied {
                commission: *row_commission(grant)?,
                name: None,
                level: node.level,
            };
            Ok(Ok(applied))
        }
    }
}

/// The existing commission a rename's or change's grant names.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
fn row_commission(grant: &Grant) -> anyhow::Result<&CommissionId> {
    match grant.kind() {
        crate::den::authority::GrantKind::Existing { commission, .. } => Ok(commission),
        crate::den::authority::GrantKind::Create { .. } => {
            Err(facts_lied("an existing node's act holds a create grant"))
        }
    }
}

/// What a handler lent its capability drafted, with the commission its capability names.
type Lent = Option<(EntryDraft, CommissionId)>;

/// A handler's answer as the gate's: its refusal a refusal, its store failure a failure.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
fn handled(handled: Result<Option<EntryDraft>, HandlerError>) -> Gated<Option<EntryDraft>> {
    match handled {
        Ok(draft) => Ok(Ok(draft)),
        Err(HandlerError::Refused) => Ok(Err(DenRefusal::TypeRefused)),
        Err(HandlerError::Store(error)) => Err(error),
    }
}

/// Issues the probe's capability in role `R` and lends it, with the commission repo alone, to
/// the probe's handler.
#[cfg(test)]
async fn lend_probe<R: RoleTable>(
    probe: &crate::den::probe::Probe,
    grant: &Grant,
    unit: Unit<'_>,
) -> Gated<Lent> {
    let Some(capability) = issue::<R>(grant) else {
        return Ok(Err(DenRefusal::WrongGrant));
    };
    let mut repo = unit.commissions();
    let draft = match handled(probe.handle(&mut *repo, &capability).await)? {
        Ok(draft) => draft,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let commission = R::commission(capability.thing());
    Ok(Ok(draft.map(|draft| (draft, commission))))
}

/// Issues the act's capability and lends it, with the commission repo alone, to the type's
/// handler: an exhaustive match over the den's acts.
#[cfg_attr(
    not(test),
    expect(
        unused_variables,
        reason = "no act but the test probe exists until the next slice"
    )
)]
async fn lend(act: DenAct, grant: &Grant, unit: Unit<'_>) -> Gated<Lent> {
    match act {
        #[cfg(test)]
        DenAct::Probe(probe) => {
            use crate::den::probe::ProbeRole;
            match probe.role() {
                ProbeRole::Participant => lend_probe::<AsParticipant>(&probe, grant, unit).await,
                ProbeRole::Owner => lend_probe::<AsOwner>(&probe, grant, unit).await,
                ProbeRole::Creator => lend_probe::<Creator>(&probe, grant, unit).await,
            }
        }
    }
}

/// Whether `grant` is the kind `shape` needs: a create grant for a create, an existing
/// thing's for a rename or a change.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
fn grant_fits(grant: &Grant, shape: &Shape) -> bool {
    let creates = grant.minted().is_some();
    match shape {
        Shape::Create { .. } => creates,
        Shape::Rename { .. } | Shape::Change { .. } => !creates,
    }
}

/// The gate, run once per act from the write host's apply, in this order: the grant's kind
/// against the act; no made node wider than Private until 0.6; the node rows walked and
/// written under the unit's one node-port binding; the capability issued and lent to the
/// type's handler; and the handler's draft appended as the changelog entry, on the
/// capability's commission, by the grant's actor, at `now`.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "no driver calls the den's write until the next slice"
    )
)]
#[cfg_attr(
    not(test),
    expect(
        unused_variables,
        reason = "with no act but the test probe, the checked act is uninhabited outside tests"
    )
)]
pub(in crate::den) async fn gate(
    unit: Unit<'_>,
    grant: &Grant,
    checked: Checked<DenAct>,
    now: DateTimeUtc,
) -> Gated<DenApplied> {
    if !grant_fits(grant, &checked.act().shape) {
        return Ok(Err(DenRefusal::WrongGrant));
    }
    let born: Vec<(Born, atta::Level)> = checked
        .born()
        .map(|(born, level)| (born.clone(), level))
        .collect();
    if born.iter().any(|(_, level)| *level != atta::Level::Private) {
        return Ok(Err(DenRefusal::WiderThanPrivate));
    }
    let act = checked.into_act();

    #[expect(
        clippy::disallowed_methods,
        reason = "the den's write gate is the one place that reaches the node port"
    )]
    let mut rows = unit.nodes();
    let written = write_rows(&mut *rows, grant, &act.shape, &born).await;
    drop(rows);
    let applied = match written? {
        Ok(applied) => applied,
        Err(refusal) => return Ok(Err(refusal)),
    };

    let lent = match lend(act.body, grant, &mut *unit).await? {
        Ok(lent) => lent,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Some((draft, commission)) = lent {
        let EntryDraft { kind, payload } = draft;
        let entry = NewChangelogEntry::event(commission, kind, grant.actor().clone(), payload, now);
        unit.changelog().append(&entry).await?;
    }
    Ok(Ok(applied))
}

#[cfg(test)]
mod tests;
