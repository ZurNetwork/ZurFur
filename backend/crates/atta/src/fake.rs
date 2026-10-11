//! A host over an in-memory tree, recording every call, for Atta's own tests.

use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use crate::{
    Access, Admission, Entry, Found, Host, HostError, Kind, Level, Lift, Metadata, Mount, NodeName,
    Reach, RealPath, Relationship, Scope, Segment, Type, View, Viewer,
};

/// One call the fake host received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    Child {
        parent: RealPath,
        key: Segment,
    },
    Children {
        parent: RealPath,
    },
    Mounts {
        folder: RealPath,
    },
    Admit {
        path: RealPath,
        access: Access,
        reach: Reach,
    },
}

/// The shape of a [`Call`] with its paths reduced to their depth, for comparing call patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallShape {
    Child(usize),
    Children(usize),
    Mounts(usize),
    Admit(usize, Reach),
}

impl Call {
    /// This call's shape.
    pub fn shape(&self) -> CallShape {
        match self {
            Call::Child { parent, .. } => CallShape::Child(parent.depth()),
            Call::Children { parent } => CallShape::Children(parent.depth()),
            Call::Mounts { folder } => CallShape::Mounts(folder.depth()),
            Call::Admit { path, reach, .. } => CallShape::Admit(path.depth(), *reach),
        }
    }
}

/// An in-memory tree the tests build node by node.
#[derive(Default)]
pub struct FakeHost {
    nodes: Vec<(RealPath, Found)>,
    mounts: HashMap<RealPath, Vec<Mount>>,
    files: HashSet<Type>,
    refused: HashSet<(RealPath, Reach)>,
    failing: bool,
    calls: Mutex<Vec<Call>>,
}

/// A segment from test text.
pub fn segment(text: &str) -> Segment {
    Segment::try_from(text).expect("a valid test segment")
}

/// A real path from `/`-separated test text, such as `/commission/c1`.
pub fn path(text: &str) -> RealPath {
    text.split('/')
        .filter(|piece| !piece.is_empty())
        .map(segment)
        .collect()
}

/// Standing over a node's whole subtree.
pub const STANDING: Lift = Lift {
    level: Level::Public,
    scope: Scope::Subtree,
    relationship: Relationship::Standing,
};

/// A grant that opens a node alone.
pub const GRANT_NODE: Lift = Lift {
    level: Level::Public,
    scope: Scope::Node,
    relationship: Relationship::Grant,
};

/// A grant that shows a node's card alone.
pub const GRANT_CARD: Lift = Lift {
    level: Level::Listed,
    scope: Scope::Node,
    relationship: Relationship::Grant,
};

impl FakeHost {
    /// Adds a node at `at`, of type `typ`, with its own `level` and the viewer's `lifts` there.
    pub fn node(mut self, at: &str, typ: &str, level: Level, lifts: &[Lift]) -> Self {
        let path = path(at);
        let (key, parent) = path.segments().split_last().expect("a node below the root");
        let name = NodeName::try_from(format!("name of {key}")).expect("a valid test name");
        let metadata = Metadata {
            key: key.clone(),
            parent: Some(parent.iter().cloned().collect()),
            typ: Type::from(typ.to_owned()),
            level,
            name,
            symlink_target: None,
        };
        let found = Found {
            metadata,
            lifts: lifts.to_vec(),
        };
        self.nodes.push((path.clone(), found));
        self
    }

    /// Changes the display name of the node at `at`.
    pub fn named(mut self, at: &str, name: &str) -> Self {
        let name = NodeName::try_from(name).expect("a valid test name");
        self.found_mut(at).metadata.name = name;
        self
    }

    /// Makes the node at `at` a symlink to `target`.
    pub fn linked(mut self, at: &str, target: &str) -> Self {
        self.found_mut(at).metadata.symlink_target = Some(path(target));
        self
    }

    /// Makes the host answer for the node at `at` as though it lived in `parent`.
    pub fn misplaced(mut self, at: &str, parent: &str) -> Self {
        self.found_mut(at).metadata.parent = Some(path(parent));
        self
    }

    /// Adds the four top-level directories the tests use.
    pub fn top_level(self) -> Self {
        ["user", "account", "character", "commission"]
            .into_iter()
            .fold(self, |host, name| {
                host.node(&format!("/{name}"), "top", Level::Public, &[])
            })
    }

    /// Gives the viewer a mount at `segment` in `folder`, grafting in `target`.
    pub fn mount(mut self, folder: &str, segment_text: &str, target: &str) -> Self {
        let mount = Mount {
            segment: segment(segment_text),
            target: path(target),
        };
        self.mounts.entry(path(folder)).or_default().push(mount);
        self
    }

    /// Makes nodes of `typ` files.
    pub fn file_type(mut self, typ: &str) -> Self {
        self.files.insert(Type::from(typ.to_owned()));
        self
    }

    /// Makes the host refuse the node at `at` for `reach`.
    pub fn refuse(mut self, at: &str, reach: Reach) -> Self {
        self.refused.insert((path(at), reach));
        self
    }

    /// Makes every host call fail.
    pub fn failing(mut self) -> Self {
        self.failing = true;
        self
    }

    /// Every call so far, in order.
    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().expect("the call log").clone()
    }

    /// Every admit call so far, as (path, access, reach).
    pub fn admits(&self) -> Vec<(RealPath, Access, Reach)> {
        let admit = |call: Call| match call {
            Call::Admit {
                path,
                access,
                reach,
            } => Some((path, access, reach)),
            _ => None,
        };
        self.calls().into_iter().filter_map(admit).collect()
    }

    fn found_mut(&mut self, at: &str) -> &mut Found {
        let path = path(at);
        let (_, found) = self
            .nodes
            .iter_mut()
            .find(|(true_path, _)| *true_path == path)
            .expect("a node the test added");
        found
    }

    fn record(&self, call: Call) -> Result<(), HostError> {
        self.calls.lock().expect("the call log").push(call);
        if self.failing {
            let failure: Box<dyn std::error::Error + Send + Sync> = "the fake store is down".into();
            return Err(HostError::from(failure));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl Host for FakeHost {
    async fn child(
        &self,
        _viewer: &Viewer,
        parent: &RealPath,
        key: &Segment,
    ) -> Result<Option<Found>, HostError> {
        let call = Call::Child {
            parent: parent.clone(),
            key: key.clone(),
        };
        self.record(call)?;
        let wanted = parent.join(key.clone());
        let child = self
            .nodes
            .iter()
            .find(|(true_path, _)| *true_path == wanted)
            .map(|(_, found)| found.clone());
        Ok(child)
    }

    async fn children(&self, _viewer: &Viewer, parent: &RealPath) -> Result<Vec<Found>, HostError> {
        let call = Call::Children {
            parent: parent.clone(),
        };
        self.record(call)?;
        let in_parent = |true_path: &RealPath| {
            true_path
                .segments()
                .split_last()
                .is_some_and(|(_, above)| above == parent.segments())
        };
        let children = self
            .nodes
            .iter()
            .filter(|(true_path, _)| in_parent(true_path))
            .map(|(_, found)| found.clone())
            .collect();
        Ok(children)
    }

    async fn mounts(&self, _viewer: &Viewer, folder: &RealPath) -> Result<Vec<Mount>, HostError> {
        let call = Call::Mounts {
            folder: folder.clone(),
        };
        self.record(call)?;
        Ok(self.mounts.get(folder).cloned().unwrap_or_default())
    }

    fn kind(&self, typ: &Type) -> Kind {
        if self.files.contains(typ) {
            Kind::File
        } else {
            Kind::Directory
        }
    }

    async fn admit(
        &self,
        _viewer: &Viewer,
        path: &RealPath,
        _node: &Metadata,
        access: Access,
        reach: Reach,
    ) -> Result<Admission, HostError> {
        let call = Call::Admit {
            path: path.clone(),
            access,
            reach,
        };
        self.record(call)?;
        let refused = self.refused.contains(&(path.clone(), reach));
        let admission = if refused {
            Admission::Refuse
        } else {
            Admission::Admit
        };
        Ok(admission)
    }
}

/// The own level an entry shows, if the viewer can open it.
pub fn own_level(entry: &Entry) -> Option<Level> {
    match &entry.view {
        View::Open { own_level, .. } => Some(*own_level),
        View::Card => None,
    }
}

/// The Den path an entry shows, if the viewer can open it.
pub fn den_path(entry: &Entry) -> Option<&[Segment]> {
    match &entry.view {
        View::Open { den_path, .. } => Some(den_path),
        View::Card => None,
    }
}

/// Where an entry really lives, if the viewer can open it.
pub fn real_path(entry: &Entry) -> Option<&RealPath> {
    match &entry.view {
        View::Open { real_path, .. } => Some(real_path),
        View::Card => None,
    }
}

/// Whether an entry is a directory or a file, if the viewer can open it.
pub fn kind(entry: &Entry) -> Option<Kind> {
    match &entry.view {
        View::Open { kind, .. } => Some(*kind),
        View::Card => None,
    }
}
