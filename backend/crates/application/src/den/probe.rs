//! The test-only probe act: a den act whose shape, anchor, type, needed role, picked level and
//! handler a test sets, so the write's every path can be driven before a real act exists.

use domain::{
    elements::{
        commission::{
            AsOwner, AsParticipant, ChangelogEntryKind, Commission, CommissionId, CommissionRole,
            NewCommission,
        },
        name::NodeName,
    },
    ports::Creator,
};

use atta::{Act, ActName, Born, Placed, RealPath, Shape};

use crate::{
    commission::den::CommissionPlace,
    den::{
        DenType, Handler, Lent, Place, TopLevel,
        act::{ActShape, ActSpec, Anchor, DenAct, Handled, HandlerError},
        host::type_key,
        names, needs,
        write::commission_directory,
    },
};

/// What the probe's handler does once lent its view.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ProbeHandler {
    /// Succeeds: a creator's creates the commission; each answers the lent commission.
    Succeed,
    /// Refuses the act as its type.
    Refuse,
    /// Fails as a store under it would.
    Fail,
    /// A creator's reads the commission before it exists and drops that miss, then creates it
    /// and answers it.
    SwallowMiss,
    /// A creator's creates the commission and reads it, creates it again and drops that
    /// store failure, then answers what it read.
    SwallowStoreFailure,
}

/// Which capability the probe is lent: the role marker its act needs, or a create's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeRole {
    /// Any Participant's.
    Participant,
    /// The Owner's.
    Owner,
    /// A creator's.
    Creator,
}

/// The probe act.
#[derive(Debug, Clone)]
pub(crate) struct Probe {
    /// Where it is anchored.
    pub(crate) anchor: Anchor,
    /// What it does.
    pub(crate) shape: ActShape,
    /// The type it acts on or makes.
    pub(crate) den_type: DenType,
    /// The least role it needs on an existing commission.
    pub(crate) needs: CommissionRole,
    /// What its handler does.
    pub(crate) handler: ProbeHandler,
}

/// A name from test text.
pub(crate) fn name(text: &str) -> NodeName {
    NodeName::try_from(text).expect("a valid test name")
}

impl Probe {
    /// A probe on the commission `commission`, needing a Participant, whose handler succeeds.
    fn on(commission: CommissionId, shape: ActShape) -> Self {
        let place = Place::Commission(CommissionPlace::Commission(commission));
        Self {
            anchor: Anchor::Thing(place),
            shape,
            den_type: DenType::Commission,
            needs: CommissionRole::Participant,
            handler: ProbeHandler::Succeed,
        }
    }

    /// Renames the commission `commission` to `text`.
    pub(crate) fn rename(commission: CommissionId, text: &str) -> Self {
        let shape = ActShape::Rename { name: name(text) };
        Self::on(commission, shape)
    }

    /// Changes the commission `commission`.
    pub(crate) fn change(commission: CommissionId) -> Self {
        Self::on(commission, ActShape::Change)
    }

    /// Creates a commission named `text` in `/commission`, holding no entries.
    pub(crate) fn create(text: &str) -> Self {
        let shape = ActShape::Create {
            name: name(text),
            level: None,
            entries: Vec::new(),
        };
        Self {
            anchor: Anchor::TopLevel(TopLevel::Commission),
            shape,
            den_type: DenType::Commission,
            needs: CommissionRole::Participant,
            handler: ProbeHandler::Succeed,
        }
    }

    /// The same probe, anchored at `anchor`.
    pub(crate) fn anchored(self, anchor: Anchor) -> Self {
        Self { anchor, ..self }
    }

    /// The same probe, needing `needs`.
    pub(crate) fn needing(self, needs: CommissionRole) -> Self {
        Self { needs, ..self }
    }

    /// The same probe, acting on or making a node of `den_type`.
    pub(crate) fn typed(self, den_type: DenType) -> Self {
        Self { den_type, ..self }
    }

    /// The same probe, its handler doing `handler`.
    pub(crate) fn handled(self, handler: ProbeHandler) -> Self {
        Self { handler, ..self }
    }

    /// The probe as the den's act.
    pub(crate) fn act(self) -> DenAct {
        DenAct::Probe(self)
    }

    /// What it declares about itself; the role it needs is its lent capability's.
    pub(crate) fn spec(&self) -> ActSpec {
        let needs = match self.role() {
            ProbeRole::Participant => needs::<ProbeAsParticipant<'_>>(),
            ProbeRole::Owner => needs::<ProbeAsOwner<'_>>(),
            ProbeRole::Creator => needs::<ProbeCreating<'_>>(),
        };
        ActSpec {
            word: "probe",
            den_type: self.den_type,
            anchor: self.anchor.clone(),
            needs,
            shape: self.shape.clone(),
        }
    }

    /// Which capability it is lent: a creator's for a create, else its need's.
    pub(crate) fn role(&self) -> ProbeRole {
        if matches!(self.shape, ActShape::Create { .. }) {
            return ProbeRole::Creator;
        }
        match self.needs {
            CommissionRole::Participant => ProbeRole::Participant,
            CommissionRole::Owner => ProbeRole::Owner,
        }
    }

    /// Its handler's answer from what its view read: a refusal or a failure as set, else the
    /// commission read.
    fn answer(&self, read: anyhow::Result<Commission>) -> Result<Handled, HandlerError> {
        match self.handler {
            ProbeHandler::Refuse => Err(HandlerError::Refused),
            ProbeHandler::Fail => Err(HandlerError::Store(anyhow::anyhow!("the probe failed"))),
            ProbeHandler::Succeed
            | ProbeHandler::SwallowMiss
            | ProbeHandler::SwallowStoreFailure => {
                let answer = read?;
                Ok(Handled { answer })
            }
        }
    }
}

/// The probe lent a Participant's capability.
pub(crate) struct ProbeAsParticipant<'p>(pub(crate) &'p Probe);

impl Handler for ProbeAsParticipant<'_> {
    type Role = AsParticipant;

    const ENTRY: ChangelogEntryKind = ChangelogEntryKind::Renamed;

    async fn handle(&self, lent: &mut Lent<'_, AsParticipant>) -> Result<Handled, HandlerError> {
        let read = lent.lent().await;
        self.0.answer(read)
    }
}

/// The probe lent the Owner's capability.
pub(crate) struct ProbeAsOwner<'p>(pub(crate) &'p Probe);

impl Handler for ProbeAsOwner<'_> {
    type Role = AsOwner;

    const ENTRY: ChangelogEntryKind = ChangelogEntryKind::Renamed;

    async fn handle(&self, lent: &mut Lent<'_, AsOwner>) -> Result<Handled, HandlerError> {
        let read = lent.lent().await;
        self.0.answer(read)
    }
}

/// The probe lent a creator's capability.
pub(crate) struct ProbeCreating<'p>(pub(crate) &'p Probe);

impl Handler for ProbeCreating<'_> {
    type Role = Creator;

    const ENTRY: ChangelogEntryKind = ChangelogEntryKind::Created;

    async fn handle(&self, lent: &mut Lent<'_, Creator>) -> Result<Handled, HandlerError> {
        let chosen = NewCommission {
            maturity: None,
            deadline: None,
        };
        match self.0.handler {
            ProbeHandler::SwallowMiss => {
                let _dropped = lent.lent().await;
                lent.create(&chosen).await?;
            }
            ProbeHandler::SwallowStoreFailure => {
                lent.create(&chosen).await?;
                let read = lent.lent().await;
                let _dropped = lent.create(&chosen).await;
                return self.0.answer(read);
            }
            ProbeHandler::Succeed => lent.create(&chosen).await?,
            ProbeHandler::Refuse | ProbeHandler::Fail => {}
        }
        let read = lent.lent().await;
        self.0.answer(read)
    }
}

/// Atta's act for `probe` built by hand, renaming the node at `path`: for the tests that drive
/// the write host below the den's builder.
pub(crate) fn hand_built_rename(path: RealPath, text: &str, probe: Probe) -> Act<DenAct> {
    let typ = type_key(probe.den_type);
    let node = Placed {
        path,
        typ: typ.clone(),
        level: atta::Level::Private,
    };
    let shape = Shape::Rename {
        node,
        name: names::display_name(&name(text)),
    };
    Act {
        name: ActName::from("probe".to_owned()),
        typ,
        shape,
        body: probe.act(),
    }
}

/// Atta's act for `probe` built by hand, creating a commission keyed `commission` in
/// `/commission`: for the tests that drive the write host below the den's builder.
pub(crate) fn hand_built_create(commission: CommissionId, text: &str, probe: Probe) -> Act<DenAct> {
    let typ = probe.den_type;
    hand_built_create_of(commission, text, typ, None, probe.act())
}

/// Atta's act built by hand around `body`, creating a node of `den_type` keyed `commission` in
/// `/commission`, named `text`, at the level `picked`, holding no entries.
pub(crate) fn hand_built_create_of(
    commission: CommissionId,
    text: &str,
    den_type: DenType,
    picked: Option<atta::Level>,
    body: DenAct,
) -> Act<DenAct> {
    let typ = type_key(den_type);
    let parent = Placed {
        path: commission_directory().expect("`commission` is a segment"),
        typ: type_key(DenType::TopLevel),
        level: atta::Level::Public,
    };
    let node = Born {
        key: names::id_segment(commission).expect("an id is a key"),
        typ: typ.clone(),
        name: names::display_name(&name(text)),
        level: picked,
    };
    let shape = Shape::Create {
        parent,
        node,
        entries: Vec::new(),
    };
    Act {
        name: ActName::from("probe".to_owned()),
        typ,
        shape,
        body,
    }
}

/// The create act for a commission titled `text`, choosing no maturity and no deadline.
pub(crate) fn create_act(text: &str) -> DenAct {
    let act = crate::commission::create::Act {
        title: text.parse().expect("a valid test title"),
        chosen: NewCommission {
            maturity: None,
            deadline: None,
        },
    };
    DenAct::Commission(crate::commission::act::CommissionAct::Create(act))
}

/// The real path of the commission `commission`.
pub(crate) fn commission_path(commission: CommissionId) -> RealPath {
    Place::Commission(CommissionPlace::Commission(commission))
        .anchored_path()
        .expect("a commission has a path")
}
