//! The test-only probe act: a den act whose shape, anchor, type, needed role, picked level and
//! handler a test sets, so the write's every path can be driven before a real act exists.

use domain::{
    elements::{
        commission::{ChangelogEntryKind, CommissionId, CommissionRole},
        level::Level,
        name::NodeName,
    },
    ports::{CommissionRepo, WriteCapability},
};

use atta::{Act, ActName, Born, Placed, RealPath, Shape};

use crate::{
    commission::den::CommissionPlace,
    den::{
        DenType, Place, TopLevel,
        act::{ActShape, ActSpec, Anchor, BornEntry, DenAct, EntryDraft, HandlerError},
        host::type_key,
        names,
        write::commission_directory,
    },
};

/// What the probe's handler does once lent its capability.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ProbeHandler {
    /// Succeeds and drafts no entry.
    Succeed,
    /// Refuses the act as its type.
    Refuse,
    /// Fails as a store under it would.
    Fail,
    /// Succeeds and drafts this entry.
    Draft(EntryDraft),
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

/// A draft entry of `kind` with `payload`.
pub(crate) fn draft(kind: ChangelogEntryKind, payload: serde_json::Value) -> EntryDraft {
    EntryDraft { kind, payload }
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

    /// The same create, its node at the level `level` picked.
    pub(crate) fn picking(self, picked: Option<Level>) -> Self {
        let ActShape::Create { name, entries, .. } = self.shape else {
            return self;
        };
        let shape = ActShape::Create {
            name,
            level: picked,
            entries,
        };
        Self { shape, ..self }
    }

    /// The same create, holding `entries`.
    pub(crate) fn holding(self, entries: Vec<BornEntry>) -> Self {
        let ActShape::Create { name, level, .. } = self.shape else {
            return self;
        };
        let shape = ActShape::Create {
            name,
            level,
            entries,
        };
        Self { shape, ..self }
    }

    /// The probe as the den's act.
    pub(crate) fn act(self) -> DenAct {
        DenAct::Probe(self)
    }

    /// What it declares about itself.
    pub(crate) fn spec(&self) -> ActSpec {
        ActSpec {
            word: "probe",
            den_type: self.den_type,
            anchor: self.anchor.clone(),
            needs: self.needs,
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

    /// Its handler, lent the commission repo and the capability as a real one is.
    pub(crate) async fn handle<T, R>(
        &self,
        _repo: &mut dyn CommissionRepo,
        _capability: &WriteCapability<T, R>,
    ) -> Result<Option<EntryDraft>, HandlerError> {
        match &self.handler {
            ProbeHandler::Succeed => Ok(None),
            ProbeHandler::Refuse => Err(HandlerError::Refused),
            ProbeHandler::Fail => Err(HandlerError::Store(anyhow::anyhow!("the probe failed"))),
            ProbeHandler::Draft(draft) => Ok(Some(draft.clone())),
        }
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
    let typ = type_key(probe.den_type);
    let parent = Placed {
        path: commission_directory().expect("`commission` is a segment"),
        typ: type_key(DenType::TopLevel),
        level: atta::Level::Public,
    };
    let node = Born {
        key: names::id_segment(commission).expect("an id is a key"),
        typ: typ.clone(),
        name: names::display_name(&name(text)),
        level: None,
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
        body: probe.act(),
    }
}

/// The real path of the commission `commission`.
pub(crate) fn commission_path(commission: CommissionId) -> RealPath {
    Place::Commission(CommissionPlace::Commission(commission))
        .anchored_path()
        .expect("a commission has a path")
}
