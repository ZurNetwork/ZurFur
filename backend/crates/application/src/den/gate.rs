//! The den's write host and its gate: the one place a write reaches the node rows, the one
//! issuer of a write capability, the one lender of a handler's view, and the one builder of a
//! write's changelog entry. Create and rename answer from what the gate wrote and the view
//! read, never from a second read of the node.

use std::{
    fmt,
    sync::{Mutex, PoisonError},
};

use atta::{
    Access, Admission, Born, Checked, DisplayName, Found, Host, HostError, Kind, Metadata, Mount,
    Placed, Reach, RealPath, Segment, Shape, Type, Viewer,
};
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            AsOwner, AsParticipant, Commission, CommissionBirth, CommissionId, CommissionRole,
            NewChangelogEntry, NewCommission,
        },
        node::{NewNode, NodeId, NodeRow, NodeSegment, NodeType},
        text::StoredText,
        user::UserId,
    },
    ports::{CommissionRepo, Creator, New, NodeRepo, SegmentTaken, Unit, WriteCapability},
};

use crate::{
    commission::act::CommissionAct,
    den::{
        act::{ArmAnswer, DenAct, EntryRule, Handled, HandlerError},
        authority::Grant,
        host::{DenHost, host_error, type_kind},
        level::den_level,
        names,
        write::commission_directory,
    },
};

/// Why the gate refused an act it was handed; it carries nothing of the act.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::den) enum DenRefusal {
    /// The grant doesn't fit the act, or its role is below the capability's.
    WrongGrant,
    /// A made node would be wider than Private.
    WiderThanPrivate,
    /// Another node already holds the made node's key.
    SegmentTaken,
}

/// What the gate wrote: the commission written, the name Atta checked, a made node's settled
/// level or the acted-on node's level, and what the type's handler answered.
pub(in crate::den) struct DenApplied {
    /// The commission written.
    pub(in crate::den) commission: CommissionId,
    /// The name a create or a rename wrote.
    pub(in crate::den) name: Option<DisplayName>,
    /// A made node's settled level, or the level of the node acted on.
    pub(in crate::den) level: atta::Level,
    /// The thing the handler acted on, as its lent view read it.
    pub(in crate::den) answer: ArmAnswer,
}

impl fmt::Debug for DenApplied {
    /// The commission, the level and the answer's id; never the name, nor the owner's DID.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DenApplied")
            .field("commission", &self.commission)
            .field("level", &self.level)
            .field("answer", &self.answer)
            .finish_non_exhaustive()
    }
}

/// What a gate step answers: a store failure as `Err`, the gate's refusal inside it.
type Gated<T> = anyhow::Result<Result<T, DenRefusal>>;

/// The host Atta hands a checked act to, for one write: it serves no read and applies acts
/// only through the gate. Built once, by the den's write.
pub(in crate::den) struct DenWriteHost<'u> {
    viewer: Viewer,
    unit: Mutex<Unit<'u>>,
    grant: Grant,
    now: DateTimeUtc,
}

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
pub(crate) trait RoleTable: Sized {
    /// What the capability is over.
    type Thing;

    /// The least role on an existing commission this role needs; `None` for a creator's.
    const NEEDS: Option<CommissionRole>;

    /// The thing `grant` leaves this role to write, or `None` when it doesn't.
    fn granted(grant: &Grant) -> Option<Self::Thing>;

    /// The commission `thing` names.
    fn commission(thing: &Self::Thing) -> CommissionId;
}

impl RoleTable for AsParticipant {
    type Thing = CommissionId;

    /// Any Participant, the Owner included.
    const NEEDS: Option<CommissionRole> = Some(CommissionRole::Participant);

    fn granted(grant: &Grant) -> Option<CommissionId> {
        Self::NEEDS.and_then(|needs| grant.existing_at_least(needs))
    }

    fn commission(thing: &CommissionId) -> CommissionId {
        *thing
    }
}

impl RoleTable for AsOwner {
    type Thing = CommissionId;

    /// The Owner only.
    const NEEDS: Option<CommissionRole> = Some(CommissionRole::Owner);

    fn granted(grant: &Grant) -> Option<CommissionId> {
        Self::NEEDS.and_then(|needs| grant.existing_at_least(needs))
    }

    fn commission(thing: &CommissionId) -> CommissionId {
        *thing
    }
}

impl RoleTable for Creator {
    type Thing = New<CommissionId>;

    /// No role: a create needs a create grant, never a standing.
    const NEEDS: Option<CommissionRole> = None;

    fn granted(grant: &Grant) -> Option<New<CommissionId>> {
        grant.minted()
    }

    fn commission(thing: &New<CommissionId>) -> CommissionId {
        *thing.as_ref()
    }
}

/// A den act's handler: the role of the capability the gate issues it, the rule for the entry
/// its act writes, and what it does with the view the gate lends it.
pub(crate) trait Handler {
    /// The role of its capability; the act's needed role is read from it, never written apart.
    type Role: RoleTable;

    /// When the gate writes the act's one changelog entry, and the gated entry it builds: a
    /// kind with its own domain constructor.
    const ENTRY: EntryRule;

    /// Acts through `lent` alone, and answers the thing it acted on as `lent` read it.
    async fn handle(&self, lent: &mut Lent<'_, Self::Role>) -> Result<Handled, HandlerError>;
}

/// The role an act handled by `H` needs on an existing commission: its capability's role's.
pub(crate) fn needs<H: Handler>() -> Option<CommissionRole> {
    <H::Role as RoleTable>::NEEDS
}

/// The write capability in role `R` that `grant` leaves, or `None`; the one issuer in the
/// workspace. A function, not `TryFrom<&Grant>`: the seam scan refuses any impl on the
/// capability outside its own file, and allows the one `gate_issued` call only in `issue`.
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
fn segment_taken(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| cause.is::<SegmentTaken>())
}

/// A failure of the den's own rules: the facts the act was built from don't match the rows.
fn facts_lied(what: &'static str) -> anyhow::Error {
    anyhow::anyhow!(what)
}

/// The node type a den type is stored as; a type the node rows don't hold is a broken rule.
fn node_type(typ: &Type) -> anyhow::Result<NodeType> {
    typ.as_ref()
        .parse::<NodeType>()
        .map_err(|_| facts_lied("the act's type is no node type"))
}

/// A segment as the node rows hold it.
fn node_segment(segment: &Segment) -> anyhow::Result<NodeSegment> {
    NodeSegment::try_from(segment.as_ref()).map_err(anyhow::Error::new)
}

/// The row of the existing node the act acts on, walked in the unit from the grant's
/// commission: `/commission/<the grant's id>`, which must be the node the grant is bound to,
/// then each deeper step of `placed`'s path. A row missing, another node than the bound one,
/// or a row of another type than `placed`'s, means the facts lied.
async fn walk(rows: &mut dyn NodeRepo, grant: &Grant, placed: &Placed) -> anyhow::Result<NodeRow> {
    let commission = grant
        .existing_at_least(CommissionRole::Participant)
        .ok_or_else(|| facts_lied("an existing node's act holds no existing grant"))?;
    let bound = grant
        .bound_node()
        .ok_or_else(|| facts_lied("an existing node's grant is bound to no node"))?;
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
    if row.id != bound {
        return Err(facts_lied(
            "the granted commission's node row is not the bound node",
        ));
    }
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
async fn create(rows: &mut dyn NodeRepo, node: &NewNode) -> Gated<()> {
    match rows.create(node).await {
        Ok(()) => Ok(Ok(())),
        Err(error) if segment_taken(&error) => Ok(Err(DenRefusal::SegmentTaken)),
        Err(error) => Err(error),
    }
}

/// What the gate wrote to the node rows: the node acted on or made, the commission the grant
/// names, the name Atta checked, a made node's settled level or the acted-on node's, and
/// whether a row changed.
struct Written {
    /// The node acted on, or the node a create made.
    node: NodeId,
    /// The grant's commission: an existing one, or the id the den minted.
    commission: CommissionId,
    /// The name a create or a rename left on the node.
    name: Option<DisplayName>,
    /// A made node's settled level, or the level of the node acted on.
    level: atta::Level,
    /// Whether the act changed a row: a create always, a rename to the stored name never.
    changed: bool,
}

/// Writes a create's rows: its node at the top under the grant's minted id, then its entries
/// under it, each at the level Atta settled. `/commission` is a rule, not a row.
async fn write_create(
    rows: &mut dyn NodeRepo,
    grant: &Grant,
    parent: &Placed,
    born: &[(Born, atta::Level)],
) -> Gated<Written> {
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
    let written = Written {
        node: made.id,
        commission,
        name: Some(node.name.clone()),
        level: *level,
        changed: true,
    };
    Ok(Ok(written))
}

/// Writes a rename's new name on the walked row, unless the row already holds it; then it
/// writes nothing, and nothing changed.
async fn write_rename(
    rows: &mut dyn NodeRepo,
    grant: &Grant,
    node: &Placed,
    name: &DisplayName,
) -> Gated<Written> {
    let row = walk(rows, grant, node).await?;
    let changed = row.name.as_str() != name.as_ref();
    if changed {
        let stored = StoredText::from(name.as_ref().to_owned());
        if !rows.set_name(&row.id, &stored).await? {
            return Err(facts_lied("a walked row is gone"));
        }
    }
    let written = Written {
        node: row.id,
        commission: *row_commission(grant)?,
        name: Some(name.clone()),
        level: node.level,
        changed,
    };
    Ok(Ok(written))
}

/// Writes the rows of `shape` from Atta's checked act: a create's node and entries, a
/// rename's new name on the walked row. No den act changes a node's content, so a change is
/// a broken rule.
async fn write_rows(
    rows: &mut dyn NodeRepo,
    grant: &Grant,
    shape: &Shape,
    born: &[(Born, atta::Level)],
) -> Gated<Written> {
    match shape {
        Shape::Create { parent, .. } => write_create(rows, grant, parent, born).await,
        Shape::Rename { node, name } => write_rename(rows, grant, node, name).await,
        Shape::Change { .. } => Err(facts_lied("no den act changes a node's content")),
    }
}

/// The existing commission a rename's grant names.
fn row_commission(grant: &Grant) -> anyhow::Result<&CommissionId> {
    match grant.kind() {
        crate::den::authority::GrantKind::Existing { commission, .. } => Ok(commission),
        crate::den::authority::GrantKind::Create { .. } => {
            Err(facts_lied("an existing node's act holds a create grant"))
        }
    }
}

/// The view's failure that its handler doesn't see: a lent call failed, and the act fails.
#[derive(Debug, thiserror::Error)]
#[error("a call under the lent view failed; the act fails")]
struct Tripped;

/// The first failure under a lent view, kept by the gate: once tripped, the act fails whatever
/// its handler answers, so a dropped `Err` still rolls the unit back.
#[derive(Default)]
struct Latch(Option<anyhow::Error>);

impl Latch {
    /// `result` as the handler gets it: its failure kept here, the handler handed [`Tripped`].
    fn pass<T>(&mut self, result: anyhow::Result<T>) -> anyhow::Result<T> {
        result.map_err(|failure| {
            self.0.get_or_insert(failure);
            anyhow::Error::new(Tripped)
        })
    }

    /// The first failure kept, if the latch tripped.
    fn tripped(self) -> Option<anyhow::Error> {
        self.0
    }
}

/// The lent view's own calls, unlatched: the commission repo, the capability, the node the gate
/// wrote, the grant's actor and the gate's time. Only [`Lent`] reaches it.
struct View<'a, R: RoleTable> {
    repo: Box<dyn CommissionRepo + 'a>,
    capability: WriteCapability<R::Thing, R>,
    node: NodeId,
    actor: UserId,
    now: DateTimeUtc,
}

impl View<'_, Creator> {
    /// Creates the capability's commission at the gate's node, owned by the grant's actor, at
    /// the gate's time, with what `commission` chose.
    async fn create(&mut self, commission: &NewCommission) -> anyhow::Result<()> {
        let birth = CommissionBirth {
            node: self.node,
            owner_id: self.actor.clone(),
            created_at: self.now,
        };
        self.repo.create(&self.capability, &birth, commission).await
    }
}

impl<R: RoleTable> View<'_, R> {
    /// The capability's commission as the unit holds it; no row is a failure.
    async fn lent(&mut self) -> anyhow::Result<Commission> {
        let commission = R::commission(self.capability.thing());
        self.repo
            .find(&commission)
            .await?
            .ok_or_else(|| facts_lied("the lent commission has no row"))
    }
}

/// What a handler is lent for one act: a view that writes only what its act may, each call
/// through the gate's latch. Its fields are private, so only the gate makes one.
pub(crate) struct Lent<'a, R: RoleTable> {
    view: View<'a, R>,
    latch: Latch,
}

impl Lent<'_, Creator> {
    /// Creates the commission the gate minted, at the node it made, for the grant's actor, at
    /// the gate's time, with what `commission` chose.
    pub(crate) async fn create(&mut self, commission: &NewCommission) -> anyhow::Result<()> {
        self.latch.pass(self.view.create(commission).await)
    }
}

impl<R: RoleTable> Lent<'_, R> {
    /// The capability's commission as the unit holds it, read after the act's writes.
    pub(crate) async fn lent(&mut self) -> anyhow::Result<Commission> {
        self.latch.pass(self.view.lent().await)
    }
}

/// What a lent handler answered, checked by the gate: the commission it acted on, and the
/// rule for the entry its act writes.
struct Landed {
    answer: Commission,
    entry: EntryRule,
}

/// Issues `H`'s capability and lends it, in a view over the commission repo, to `handler`.
/// A tripped latch fails the act whatever the handler answered; an answer naming another
/// commission than the grant's is a broken rule.
async fn lend_as<H: Handler>(
    handler: &H,
    grant: &Grant,
    unit: Unit<'_>,
    written: &Written,
    now: DateTimeUtc,
) -> Gated<Landed> {
    let Some(capability) = issue::<H::Role>(grant) else {
        return Ok(Err(DenRefusal::WrongGrant));
    };
    let view = View {
        repo: unit.commissions(),
        capability,
        node: written.node,
        actor: grant.actor().clone(),
        now,
    };
    let mut lent = Lent {
        view,
        latch: Latch::default(),
    };
    let handled = handler.handle(&mut lent).await;
    let Lent { view, latch } = lent;
    drop(view);
    if let Some(failure) = latch.tripped() {
        return Err(failure);
    }
    let Handled { answer } = handled.map_err(|HandlerError::Store(failure)| failure)?;
    if answer.id != written.commission {
        return Err(facts_lied(
            "the handler answered another commission than the grant's",
        ));
    }
    let landed = Landed {
        answer,
        entry: H::ENTRY,
    };
    Ok(Ok(landed))
}

/// Lends each act to its type's handler: an exhaustive match over the den's acts, one
/// [`lend_as`] per leaf act.
async fn lend(
    act: DenAct,
    grant: &Grant,
    unit: Unit<'_>,
    written: &Written,
    now: DateTimeUtc,
) -> Gated<Landed> {
    match act {
        DenAct::Commission(CommissionAct::Create(create)) => {
            lend_as(&create, grant, unit, written, now).await
        }
        DenAct::Commission(CommissionAct::Rename(rename)) => {
            lend_as(&rename, grant, unit, written, now).await
        }
    }
}

/// Whether `grant` is the kind `shape` needs: a create grant for a create, an existing
/// thing's for a rename or a change.
fn grant_fits(grant: &Grant, shape: &Shape) -> bool {
    let creates = grant.minted().is_some();
    match shape {
        Shape::Create { .. } => creates,
        Shape::Rename { .. } | Shape::Change { .. } => !creates,
    }
}

/// The act's one entry, if `rule` writes one for what `written` changed: built by the rule's
/// gated entry's domain constructor on the grant's commission, by `actor`, at `now`. An entry
/// of another kind than the gated entry's, or built on another commission, by another actor
/// or at another time, is a broken rule.
fn built_entry(
    rule: EntryRule,
    written: &Written,
    actor: &UserId,
    now: DateTimeUtc,
) -> anyhow::Result<Option<NewChangelogEntry>> {
    let Some(gated) = rule.entry(written.changed) else {
        return Ok(None);
    };
    let entry = gated.construct(written.commission, actor.clone(), now);
    if entry.kind() != gated.kind() {
        return Err(facts_lied(
            "an entry's constructor built another kind than its gated entry's",
        ));
    }
    let as_granted = entry.commission_id() == written.commission
        && entry.actor_id() == Some(actor)
        && entry.created_at() == now;
    if !as_granted {
        return Err(facts_lied(
            "an entry's constructor built it off the grant's commission, actor or time",
        ));
    }
    Ok(Some(entry))
}

/// Appends the act's one entry in the unit, if `rule` writes one for what `written` changed; a
/// broken rule appends nothing.
async fn append_entry(
    unit: Unit<'_>,
    rule: EntryRule,
    written: &Written,
    actor: &UserId,
    now: DateTimeUtc,
) -> anyhow::Result<()> {
    if let Some(entry) = built_entry(rule, written, actor, now)? {
        unit.changelog().append(&entry).await?;
    }
    Ok(())
}

/// The gate, run once per act from the write host's apply: the grant's kind against the act;
/// no made node wider than Private until 0.6; the node rows, under the unit's one node-port
/// binding; the act lent to its handler; then, as the act's entry rule says of what changed,
/// its one entry, built here by the rule's gated entry's domain constructor on the grant's
/// commission, by the grant's actor, at `now`, and refused unless it carries that entry's kind.
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
    let written = match written? {
        Ok(written) => written,
        Err(refusal) => return Ok(Err(refusal)),
    };

    let landed = match lend(act.body, grant, &mut *unit, &written, now).await? {
        Ok(landed) => landed,
        Err(refusal) => return Ok(Err(refusal)),
    };
    append_entry(unit, landed.entry, &written, grant.actor(), now).await?;
    let applied = DenApplied {
        commission: written.commission,
        name: written.name,
        level: written.level,
        answer: ArmAnswer::Commission(landed.answer),
    };
    Ok(Ok(applied))
}

#[cfg(test)]
mod tests;
