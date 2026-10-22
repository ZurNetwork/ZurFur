//! The den's write host and its gate: the one place a write reaches the node rows, the one
//! issuer of a write capability, the one lender of a handler's view, and the one builder of a
//! write's changelog entry. Each act's rule runs first, on the facts its grant read, before any
//! row is written or any capability issued; each act is lent a view of its own, whose one write
//! lands its checked change exactly once. Create and rename answer from what the gate wrote and
//! the view read, never from a second read of the node.

use std::{
    fmt,
    sync::{Mutex, PoisonError},
};

use atta::{
    Access, Act, Admission, Born, Checked, DisplayName, Found, Host, HostError, Kind, Metadata,
    Mount, Placed, Reach, RealPath, Segment, Shape, Type, Viewer,
};
use domain::{
    datetime::DateTimeUtc,
    elements::{
        commission::{
            AsOwner, AsParticipant, Commission, CommissionBirth, CommissionId, CommissionRefusal,
            CommissionRole, CommissionState, NewChangelogEntry,
        },
        node::{NewNode, NodeId, NodeRow, NodeSegment, NodeType},
        text::StoredText,
        user::UserId,
    },
    ports::{CommissionRepo, Creator, New, NodeRepo, SegmentTaken, Unit, WriteCapability},
};

use crate::{
    commission::{act::CommissionAct, create, lifecycle::move_to, rename},
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
    /// The act's own domain rule refused it, on the facts its grant read.
    Precondition(CommissionRefusal),
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

/// What a create reads before it acts: nothing, since the thing isn't made yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Unmade;

/// Which grant a capability's role needs, the facts its acts' rules read, and the commission
/// its thing names: the gate's role table. A den trait because the markers are workflow's and
/// kernel's, with no trait of their own, and only this gate may read it.
pub(crate) trait RoleTable: Sized {
    /// What the capability is over.
    type Thing;

    /// What an act in this role reads before it acts.
    type Facts;

    /// The least role on an existing commission this role needs; `None` for a creator's.
    const NEEDS: Option<CommissionRole>;

    /// The thing `grant` leaves this role to write, or `None` when it doesn't.
    fn granted(grant: &Grant) -> Option<Self::Thing>;

    /// The facts `grant` carries for this role, or `None` when it leaves this role nothing.
    fn facts(grant: &Grant) -> Option<&Self::Facts>;

    /// The commission `thing` names.
    fn commission(thing: &Self::Thing) -> CommissionId;
}

impl RoleTable for AsParticipant {
    type Thing = CommissionId;

    type Facts = CommissionState;

    /// Any Participant, the Owner included.
    const NEEDS: Option<CommissionRole> = Some(CommissionRole::Participant);

    fn granted(grant: &Grant) -> Option<CommissionId> {
        Self::NEEDS.and_then(|needs| grant.existing_at_least(needs))
    }

    fn facts(grant: &Grant) -> Option<&CommissionState> {
        Self::NEEDS.and_then(|needs| grant.state_at_least(needs))
    }

    fn commission(thing: &CommissionId) -> CommissionId {
        *thing
    }
}

impl RoleTable for AsOwner {
    type Thing = CommissionId;

    type Facts = CommissionState;

    /// The Owner only.
    const NEEDS: Option<CommissionRole> = Some(CommissionRole::Owner);

    fn granted(grant: &Grant) -> Option<CommissionId> {
        Self::NEEDS.and_then(|needs| grant.existing_at_least(needs))
    }

    fn facts(grant: &Grant) -> Option<&CommissionState> {
        Self::NEEDS.and_then(|needs| grant.state_at_least(needs))
    }

    fn commission(thing: &CommissionId) -> CommissionId {
        *thing
    }
}

impl RoleTable for Creator {
    type Thing = New<CommissionId>;

    type Facts = Unmade;

    /// No role: a create needs a create grant, never a standing.
    const NEEDS: Option<CommissionRole> = None;

    fn granted(grant: &Grant) -> Option<New<CommissionId>> {
        grant.minted()
    }

    fn facts(grant: &Grant) -> Option<&Unmade> {
        grant.minted().map(|_| &Unmade)
    }

    fn commission(thing: &New<CommissionId>) -> CommissionId {
        *thing.as_ref()
    }
}

/// A den act's handler: the role of the capability the gate issues it, the change its rule
/// checks and the rule itself, the rule for the entry its act writes, and what it does with
/// the view the gate lends it.
pub(crate) trait Handler: Sized {
    /// The role of its capability; the act's needed role is read from it, never written apart.
    type Role: RoleTable;

    /// The change its rule checks, which its view's one write writes.
    type Checked;

    /// When the gate writes the act's one changelog entry, and the gated entry it builds: the
    /// kinds it may build with its own domain constructor, over the checked change.
    const ENTRY: EntryRule<Self::Checked>;

    /// The act's domain rule on the facts its grant read at `now`: the change it checks,
    /// `None` when there is nothing to write, or the rule's refusal.
    fn precondition(
        &self,
        facts: &<Self::Role as RoleTable>::Facts,
        now: DateTimeUtc,
    ) -> Result<Option<Self::Checked>, CommissionRefusal>;

    /// Acts through `lent` alone, and answers the thing it acted on as `lent` read it.
    async fn handle(&self, lent: &mut Lent<'_, Self>) -> Result<Handled, HandlerError>;
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

/// Walks a field act's node, which must be the node the grant is bound to, and writes no node
/// row: a field act changes the commission's fields through its view, never its node.
async fn write_change(rows: &mut dyn NodeRepo, grant: &Grant, node: &Placed) -> Gated<Written> {
    let row = walk(rows, grant, node).await?;
    let written = Written {
        node: row.id,
        commission: *row_commission(grant)?,
        name: None,
        level: node.level,
        changed: false,
    };
    Ok(Ok(written))
}

/// Writes the rows of `shape` from Atta's checked act: a create's node and entries, a
/// rename's new name on the walked row, and nothing for a change, whose node is only walked.
async fn write_rows(
    rows: &mut dyn NodeRepo,
    grant: &Grant,
    shape: &Shape,
    born: &[(Born, atta::Level)],
) -> Gated<Written> {
    match shape {
        Shape::Create { parent, .. } => write_create(rows, grant, parent, born).await,
        Shape::Rename { node, name } => write_rename(rows, grant, node, name).await,
        Shape::Change { node } => write_change(rows, grant, node).await,
    }
}

/// The existing commission a rename's or a change's grant names.
fn row_commission(grant: &Grant) -> anyhow::Result<&CommissionId> {
    match grant.kind() {
        crate::den::authority::GrantKind::Existing { commission, .. } => Ok(commission),
        crate::den::authority::GrantKind::Create { .. } => {
            Err(facts_lied("an existing node's act holds a create grant"))
        }
    }
}

/// An act whose rule passed, before anything is written: its handler and the change its rule
/// checked, `None` when there is nothing to write.
struct Passed<H: Handler> {
    handler: H,
    change: Option<H::Checked>,
}

/// Each act whose rule passed, by its handler: an exhaustive enum over the den's leaf acts.
enum Prepared {
    /// A commission's create.
    Create(Passed<create::Act>),
    /// A commission's rename.
    Rename(Passed<rename::Act>),
    /// A commission's lifecycle move.
    MoveLifecycle(Passed<move_to::Act>),
}

/// `handler`'s rule on the facts `grant` carries for its role, at `now`: the act, passed, or
/// the rule's refusal. A grant that carries no facts for the role is the wrong grant.
fn passed<H: Handler>(
    handler: H,
    grant: &Grant,
    now: DateTimeUtc,
) -> Result<Passed<H>, DenRefusal> {
    let facts = <H::Role as RoleTable>::facts(grant).ok_or(DenRefusal::WrongGrant)?;
    let change = handler
        .precondition(facts, now)
        .map_err(DenRefusal::Precondition)?;
    let passed = Passed { handler, change };
    Ok(passed)
}

/// Runs each act's rule before anything is written: an exhaustive match over the den's acts.
fn prepare(act: DenAct, grant: &Grant, now: DateTimeUtc) -> Result<Prepared, DenRefusal> {
    match act {
        DenAct::Commission(CommissionAct::Create(create)) => {
            passed(create, grant, now).map(Prepared::Create)
        }
        DenAct::Commission(CommissionAct::Rename(rename)) => {
            passed(rename, grant, now).map(Prepared::Rename)
        }
        DenAct::Commission(CommissionAct::MoveLifecycle(move_to)) => {
            passed(move_to, grant, now).map(Prepared::MoveLifecycle)
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

/// What an act's one write did the first time it was called; a second call changes nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wrote {
    /// The rule checked no change, so it wrote nothing.
    Nothing,
    /// The checked change landed on exactly one row.
    Landed,
    /// The write was made and didn't land: it failed, or its compare-and-set matched no row.
    Missed,
}

/// A second call of an act's one write: a broken rule, kept by the latch.
fn written_twice() -> anyhow::Error {
    facts_lied("an act's one write was called twice")
}

/// The lent view's own calls, unlatched: the commission repo, the capability, the node the gate
/// wrote, the grant's actor, the gate's time, the change the act's rule checked, and what its
/// one write did. Only [`Lent`] reaches it.
struct View<'a, H: Handler> {
    repo: Box<dyn CommissionRepo + 'a>,
    capability: WriteCapability<<H::Role as RoleTable>::Thing, H::Role>,
    node: NodeId,
    actor: UserId,
    now: DateTimeUtc,
    change: Option<H::Checked>,
    wrote: Option<Wrote>,
}

impl View<'_, create::Act> {
    /// Creates the capability's commission at the gate's node, owned by the grant's actor, at
    /// the gate's time, with what the create chose, once.
    async fn create(&mut self) -> anyhow::Result<()> {
        if self.wrote.is_some() {
            return Err(written_twice());
        }
        let Some(commission) = &self.change else {
            self.wrote = Some(Wrote::Nothing);
            return Ok(());
        };
        self.wrote = Some(Wrote::Missed);
        let birth = CommissionBirth {
            node: self.node,
            owner_id: self.actor.clone(),
            created_at: self.now,
        };
        self.repo
            .create(&self.capability, &birth, commission)
            .await?;
        self.wrote = Some(Wrote::Landed);
        Ok(())
    }
}

impl View<'_, move_to::Act> {
    /// Moves the capability's commission as the rule checked, by compare-and-set, once; a
    /// move that matches no row fails.
    async fn set_lifecycle(&mut self) -> anyhow::Result<()> {
        if self.wrote.is_some() {
            return Err(written_twice());
        }
        let Some(change) = &self.change else {
            self.wrote = Some(Wrote::Nothing);
            return Ok(());
        };
        self.wrote = Some(Wrote::Missed);
        let landed = self.repo.set_lifecycle(&self.capability, change).await?;
        if !landed {
            return Err(facts_lied("the lifecycle move matched no row"));
        }
        self.wrote = Some(Wrote::Landed);
        Ok(())
    }
}

impl<H: Handler> View<'_, H> {
    /// The capability's commission as the unit holds it; no row is a failure.
    async fn lent(&mut self) -> anyhow::Result<Commission> {
        let commission = <H::Role as RoleTable>::commission(self.capability.thing());
        self.repo
            .find(&commission)
            .await?
            .ok_or_else(|| facts_lied("the lent commission has no row"))
    }
}

/// What a handler is lent for one act: a view of its own act, whose one write writes only the
/// change its rule checked, each call through the gate's latch. Its fields are private, so only
/// the gate makes one.
pub(crate) struct Lent<'a, H: Handler> {
    view: View<'a, H>,
    latch: Latch,
}

impl Lent<'_, create::Act> {
    /// Creates the commission the gate minted, at the node it made, for the grant's actor, at
    /// the gate's time, with what the create chose.
    pub(crate) async fn create(&mut self) -> anyhow::Result<()> {
        self.latch.pass(self.view.create().await)
    }
}

impl Lent<'_, move_to::Act> {
    /// Moves the commission's lifecycle as the rule checked: once, and only a real move.
    pub(crate) async fn set_lifecycle(&mut self) -> anyhow::Result<()> {
        self.latch.pass(self.view.set_lifecycle().await)
    }
}

impl<H: Handler> Lent<'_, H> {
    /// The capability's commission as the unit holds it, read after the act's writes.
    pub(crate) async fn lent(&mut self) -> anyhow::Result<Commission> {
        self.latch.pass(self.view.lent().await)
    }
}

/// What a lent handler answered, checked by the gate: the commission it acted on, the change
/// its view's write landed, and the capability, kept while the entry is built.
struct Settled<H: Handler> {
    answer: Commission,
    recorded: Option<H::Checked>,
    capability: WriteCapability<<H::Role as RoleTable>::Thing, H::Role>,
}

/// The change an act's one write landed: `None` when its rule checked none and it wrote
/// nothing, the change when it landed exactly once, and a broken rule otherwise.
fn landed_once<C>(change: Option<C>, wrote: Option<Wrote>) -> anyhow::Result<Option<C>> {
    match (change, wrote) {
        (None, None | Some(Wrote::Nothing)) => Ok(None),
        (Some(change), Some(Wrote::Landed)) => Ok(Some(change)),
        (Some(_), None) => Err(facts_lied("the act's checked change was never written")),
        (Some(_), Some(Wrote::Missed)) => Err(facts_lied("the act's checked change didn't land")),
        (Some(_), Some(Wrote::Nothing)) | (None, Some(Wrote::Landed | Wrote::Missed)) => Err(
            facts_lied("the act's write wrote other than its checked change"),
        ),
    }
}

/// Takes the lent view back after its handler answered: a tripped latch fails the act whatever
/// the handler answered; an answer naming another commission than the grant's is a broken
/// rule; and a checked change must have landed exactly once.
fn settle<H: Handler>(
    lent: Lent<'_, H>,
    handled: Result<Handled, HandlerError>,
    written: &Written,
) -> anyhow::Result<Settled<H>> {
    let Lent { view, latch } = lent;
    let View {
        repo,
        capability,
        change,
        wrote,
        ..
    } = view;
    drop(repo);
    if let Some(failure) = latch.tripped() {
        return Err(failure);
    }
    let Handled { answer } = handled.map_err(|HandlerError::Store(failure)| failure)?;
    if answer.id != written.commission {
        return Err(facts_lied(
            "the handler answered another commission than the grant's",
        ));
    }
    let recorded = landed_once(change, wrote)?;
    let settled = Settled {
        answer,
        recorded,
        capability,
    };
    Ok(settled)
}

/// Issues `H`'s capability and lends it, in a view of the act's own over the commission repo
/// holding the change its rule checked, to its handler; then settles what the handler did and,
/// while the capability lives, builds and appends the act's entry from the change the view
/// recorded.
async fn lend_as<H: Handler>(
    passed: Passed<H>,
    grant: &Grant,
    unit: Unit<'_>,
    written: &Written,
    now: DateTimeUtc,
) -> Gated<Commission> {
    let Passed { handler, change } = passed;
    let Some(capability) = issue::<H::Role>(grant) else {
        return Ok(Err(DenRefusal::WrongGrant));
    };
    let view = View {
        repo: unit.commissions(),
        capability,
        node: written.node,
        actor: grant.actor().clone(),
        now,
        change,
        wrote: None,
    };
    let mut lent = Lent {
        view,
        latch: Latch::default(),
    };
    let handled = handler.handle(&mut lent).await;
    let settled = settle(lent, handled, written)?;
    let recorded = settled.recorded.as_ref();
    append_entry(unit, H::ENTRY, written, recorded, grant.actor(), now).await?;
    drop(settled.capability);
    Ok(Ok(settled.answer))
}

/// Lends each passed act to its handler: an exhaustive match over the den's acts, one
/// [`lend_as`] per leaf act.
async fn lend(
    prepared: Prepared,
    grant: &Grant,
    unit: Unit<'_>,
    written: &Written,
    now: DateTimeUtc,
) -> Gated<Commission> {
    match prepared {
        Prepared::Create(create) => lend_as(create, grant, unit, written, now).await,
        Prepared::Rename(rename) => lend_as(rename, grant, unit, written, now).await,
        Prepared::MoveLifecycle(move_to) => lend_as(move_to, grant, unit, written, now).await,
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

/// The act's one entry, if `rule` writes one for what changed (its node rows, or its view's
/// write, which recorded `recorded`): built by the rule's gated entry's domain constructor on
/// the grant's commission, by `actor`, at `now`, from the recorded change. An entry its gated
/// entry may not build, built on another commission, by another actor or at another time, or
/// carrying a note, is a broken rule.
fn built_entry<C>(
    rule: EntryRule<C>,
    written: &Written,
    recorded: Option<&C>,
    actor: &UserId,
    now: DateTimeUtc,
) -> anyhow::Result<Option<NewChangelogEntry>> {
    let changed = written.changed || recorded.is_some();
    let Some(gated) = rule.entry(changed) else {
        return Ok(None);
    };
    let entry = gated
        .construct(written.commission, actor.clone(), recorded, now)
        .ok_or_else(|| facts_lied("an entry's constructor takes a change no write recorded"))?;
    if !gated.builds(entry.kind()) {
        return Err(facts_lied(
            "an entry's constructor built a kind its gated entry doesn't name",
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
    if entry.note_text().is_some() {
        return Err(facts_lied("an entry's constructor attached a note"));
    }
    Ok(Some(entry))
}

/// Appends the act's one entry in the unit, if `rule` writes one for what changed; a broken
/// rule appends nothing.
async fn append_entry<C>(
    unit: Unit<'_>,
    rule: EntryRule<C>,
    written: &Written,
    recorded: Option<&C>,
    actor: &UserId,
    now: DateTimeUtc,
) -> anyhow::Result<()> {
    if let Some(entry) = built_entry(rule, written, recorded, actor, now)? {
        unit.changelog().append(&entry).await?;
    }
    Ok(())
}

/// The gate, run once per act from the write host's apply: the grant's kind against the act;
/// no made node wider than Private until 0.6; the act's own rule on the grant's facts, before
/// any row or capability; the node rows, under the unit's one node-port binding; then the act
/// lent to its handler, whose entry is built and appended as its rule says of what changed.
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
    let Act { shape, body, .. } = checked.into_act();
    let prepared = match prepare(body, grant, now) {
        Ok(prepared) => prepared,
        Err(refusal) => return Ok(Err(refusal)),
    };

    #[expect(
        clippy::disallowed_methods,
        reason = "the den's write gate is the one place that reaches the node port"
    )]
    let mut rows = unit.nodes();
    let written = write_rows(&mut *rows, grant, &shape, &born).await;
    drop(rows);
    let written = match written? {
        Ok(written) => written,
        Err(refusal) => return Ok(Err(refusal)),
    };

    let answer = match lend(prepared, grant, &mut *unit, &written, now).await? {
        Ok(answer) => answer,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let applied = DenApplied {
        commission: written.commission,
        name: written.name,
        level: written.level,
        answer: ArmAnswer::Commission(answer),
    };
    Ok(Ok(applied))
}

#[cfg(test)]
mod tests;
