use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Mutex, MutexGuard, PoisonError},
};

use crate::{
    Ports, account, character, commission,
    den::{
        DenContext, DenMount, DenType, Node, NodeKind, NodeState, Place, SoftDelete, TopLevel,
        admission::admission, names, place::Parent, post, read::SoftDeleteLookup,
    },
    user,
};
use atta::{
    Access, Admission, Found, Host, HostError, Kind, Level, Metadata, Mount, Reach, RealPath,
    Segment, Type, Viewer,
};
use domain::elements::{
    account::AccountId, character::CharacterId, commission::CommissionId, did::Did, user::UserId,
};

/// What the host recorded about a node it answered for in this request.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Recorded {
    place: Place,
    state: NodeState,
}

/// Zurfur's one host for Atta, built per request for the session's viewer. It records every
/// node it answers for, keyed by its real path, so `admit` and the read's soft-delete words
/// answer from that record alone.
pub(crate) struct DenHost<'a> {
    context: DenContext<'a>,
    viewer: Viewer,
    include_deleted: bool,
    record: Mutex<HashMap<RealPath, Recorded>>,
}

/// An arm's failure as the engine's host failure.
fn host_error(error: anyhow::Error) -> HostError {
    let boxed: Box<dyn std::error::Error + Send + Sync> = error.into();
    HostError::from(boxed)
}

/// An arm's answer, kept only when its place's key is the key it was asked for.
fn answered_for(node: Option<Node>, key: &Segment) -> Option<Node> {
    node.filter(|node| node.place.key().as_ref() == Some(key))
}

/// The engine's type key for a Den type.
fn type_key(den_type: DenType) -> Type {
    let word: &'static str = den_type.into();
    Type::from(word.to_owned())
}

impl<'a> DenHost<'a> {
    /// A host for `viewer`, the session's user, reading through `ports`; `include_deleted` is
    /// the request's own and never reaches the engine.
    pub(crate) fn new(ports: &'a Ports, viewer: &'a UserId, include_deleted: bool) -> Self {
        let context = DenContext { ports, viewer };
        Self {
            viewer: Self::viewer_of(context.viewer),
            context,
            include_deleted,
            record: Mutex::new(HashMap::new()),
        }
    }

    /// The engine's viewer for a session's user.
    pub(crate) fn viewer_of(user: &UserId) -> Viewer {
        Viewer::from(user.to_string())
    }

    /// The engine's viewer this host answers for.
    pub(crate) fn viewer(&self) -> &Viewer {
        &self.viewer
    }

    /// The record, never held across an await.
    fn record(&self) -> MutexGuard<'_, HashMap<RealPath, Recorded>> {
        self.record.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Where the folder at `path` sits; `None` when the host never answered for it, or when
    /// its type doesn't show its content yet, which closes typed paths below it as it empties
    /// its listing.
    fn folder(&self, path: &RealPath) -> Option<Parent> {
        if path.segments().is_empty() {
            return Some(Parent::Root);
        }
        let record = self.record();
        let recorded = record.get(path)?;
        if recorded.place.den_type().content_not_shown() {
            return None;
        }
        Some(Parent::At(recorded.place.clone()))
    }

    /// The engine's answer for an arm's node in the folder `parent` at `parent_path`, recorded
    /// under its real path; `None` when its place can't be a child of that folder or makes no
    /// key.
    fn found(&self, parent_path: &RealPath, parent: &Parent, node: Node) -> Option<Found> {
        if !node.place.is_child_of(parent) {
            return None;
        }
        let parent = parent_path;
        let key = node.place.key()?;
        let metadata = Metadata {
            key: key.clone(),
            parent: Some(parent.clone()),
            typ: type_key(node.place.den_type()),
            level: node.level,
            name: node.name,
            symlink_target: None,
        };
        let recorded = Recorded {
            place: node.place,
            state: node.state,
        };
        self.record().insert(parent.join(key), recorded);
        let found = Found {
            metadata,
            lifts: node.lifts,
        };
        Some(found)
    }

    /// The engine's mount for an arm's mount; `None` when its target doesn't sit directly
    /// under a top-level directory.
    fn mount(mount: DenMount) -> Option<Mount> {
        let target = mount.target.anchored_path()?;
        let mount = Mount {
            segment: mount.segment,
            target,
        };
        Some(mount)
    }

    /// The top-level directory named `key` in `/`: Public, owned by the system, live.
    fn top_level(key: &Segment) -> Option<Node> {
        let top_level = TopLevel::from_str(key.as_ref()).ok()?;
        let name = names::fixed_name(top_level.into())?;
        let node = Node {
            place: Place::TopLevel(top_level),
            name,
            level: Level::Public,
            lifts: Vec::new(),
            state: NodeState::Live,
        };
        Some(node)
    }

    /// The session's own user when `key` is their DID; any other user is absent to them.
    fn own_user(&self, key: &Segment) -> Option<UserId> {
        let did = Did::from_str(key.as_ref()).ok()?;
        let user = UserId::from(did);
        (&user == self.context.viewer).then_some(user)
    }

    /// The node keyed `key` directly under `top_level`, from the arm that owns its entities;
    /// a key that isn't that arm's id is absent, and so is any user but the session's.
    async fn entity(&self, top_level: TopLevel, key: &Segment) -> anyhow::Result<Option<Node>> {
        let context = &self.context;
        match top_level {
            TopLevel::User => {
                let Some(user) = self.own_user(key) else {
                    return Ok(None);
                };
                user::den::node(context, &user).await
            }
            TopLevel::Account => {
                let Ok(did) = Did::from_str(key.as_ref()) else {
                    return Ok(None);
                };
                account::den::node(context, &AccountId::from(did)).await
            }
            TopLevel::Character => {
                let Ok(did) = Did::from_str(key.as_ref()) else {
                    return Ok(None);
                };
                character::den::node(context, &CharacterId::from(did)).await
            }
            TopLevel::Commission => {
                let Ok(commission) = CommissionId::from_str(key.as_ref()) else {
                    return Ok(None);
                };
                commission::den::node(context, commission).await
            }
            TopLevel::Post => {
                let Ok(poster) = Did::from_str(key.as_ref()) else {
                    return Ok(None);
                };
                Ok(post::node(&poster))
            }
        }
    }

    /// The node at `key` in the folder at `parent`, from the arm that owns the folder.
    async fn child_at(&self, parent: &Parent, key: &Segment) -> anyhow::Result<Option<Node>> {
        let context = &self.context;
        let Parent::At(place) = parent else {
            return Ok(Self::top_level(key));
        };
        match place {
            Place::TopLevel(top_level) => self.entity(*top_level, key).await,
            Place::User(place) => user::den::child(context, place, key).await,
            Place::Account(place) => account::den::child(context, place, key).await,
            Place::Character(place) => character::den::child(context, place, key).await,
            Place::Commission(place) => commission::den::child(context, place, key).await,
            Place::Workflow(place) => account::workflow::den::child(context, place, key).await,
            Place::Post(place) => Ok(post::child(place, key)),
        }
    }

    /// Every real child of the folder at `parent`, from the arm that owns the folder. `/` and
    /// the top-level directories are traverse-only and list nothing.
    async fn children_at(&self, parent: &Parent) -> anyhow::Result<Vec<Node>> {
        let context = &self.context;
        let Parent::At(place) = parent else {
            return Ok(Vec::new());
        };
        match place {
            Place::TopLevel(_) => Ok(Vec::new()),
            Place::User(place) => user::den::children(context, place).await,
            Place::Account(place) => account::den::children(context, place).await,
            Place::Character(place) => character::den::children(context, place).await,
            Place::Commission(place) => commission::den::children(context, place).await,
            Place::Workflow(place) => account::workflow::den::children(context, place).await,
            Place::Post(place) => Ok(post::children(place)),
        }
    }

    /// The viewer's mounts in the folder at `parent`, from the arm that owns the folder. `/`
    /// and the top-level directories hold none.
    async fn mounts_at(&self, parent: &Parent) -> anyhow::Result<Vec<DenMount>> {
        let context = &self.context;
        let Parent::At(place) = parent else {
            return Ok(Vec::new());
        };
        match place {
            Place::TopLevel(_) => Ok(Vec::new()),
            Place::User(place) => user::den::mounts(context, place).await,
            Place::Account(place) => account::den::mounts(context, place).await,
            Place::Character(place) => character::den::mounts(context, place).await,
            Place::Commission(place) => commission::den::mounts(context, place).await,
            Place::Workflow(place) => account::workflow::den::mounts(context, place).await,
            Place::Post(place) => Ok(post::mounts(place)),
        }
    }
}

#[async_trait::async_trait]
impl Host for DenHost<'_> {
    /// Absent for any viewer but the session's, in a folder the host never answered for or
    /// whose content isn't shown, and when the arm answers for another key.
    async fn child(
        &self,
        viewer: &Viewer,
        parent: &RealPath,
        key: &Segment,
    ) -> Result<Option<Found>, HostError> {
        if viewer != &self.viewer {
            return Ok(None);
        }
        let Some(folder) = self.folder(parent) else {
            return Ok(None);
        };
        let node = self.child_at(&folder, key).await.map_err(host_error)?;
        let found = answered_for(node, key).and_then(|node| self.found(parent, &folder, node));
        Ok(found)
    }

    /// Empty for any viewer but the session's, and for a folder the host never answered for
    /// or whose content isn't shown.
    async fn children(&self, viewer: &Viewer, parent: &RealPath) -> Result<Vec<Found>, HostError> {
        if viewer != &self.viewer {
            return Ok(Vec::new());
        }
        let Some(folder) = self.folder(parent) else {
            return Ok(Vec::new());
        };
        let nodes = self.children_at(&folder).await.map_err(host_error)?;
        let found = nodes
            .into_iter()
            .filter_map(|node| self.found(parent, &folder, node))
            .collect();
        Ok(found)
    }

    /// Empty for any viewer but the session's, and for a folder the host never answered for
    /// or whose content isn't shown.
    async fn mounts(&self, viewer: &Viewer, folder: &RealPath) -> Result<Vec<Mount>, HostError> {
        if viewer != &self.viewer {
            return Ok(Vec::new());
        }
        let Some(parent) = self.folder(folder) else {
            return Ok(Vec::new());
        };
        let mounts = self.mounts_at(&parent).await.map_err(host_error)?;
        let mounts = mounts.into_iter().filter_map(Self::mount).collect();
        Ok(mounts)
    }

    /// A type key this host doesn't know is a file: never traversed, never listed.
    fn kind(&self, typ: &Type) -> Kind {
        let Ok(den_type) = DenType::from_str(typ.as_ref()) else {
            return Kind::File;
        };
        match den_type.kind() {
            NodeKind::Directory => Kind::Directory,
            NodeKind::File | NodeKind::Symlink => Kind::File,
        }
    }

    /// Answers from the record alone; refuses any viewer but the session's.
    fn admit(
        &self,
        viewer: &Viewer,
        path: &RealPath,
        _node: &Metadata,
        access: Access,
        reach: Reach,
    ) -> Admission {
        if viewer != &self.viewer {
            return Admission::Refuse;
        }
        let state = self.record().get(path).map(|recorded| recorded.state);
        admission(state, access, reach, self.include_deleted)
    }
}

impl SoftDeleteLookup for DenHost<'_> {
    /// The word recorded for the node this host answered for at `path`.
    fn soft_delete(&self, path: &RealPath) -> Option<Option<SoftDelete>> {
        let record = self.record();
        let recorded = record.get(path)?;
        Some(recorded.state.soft_delete())
    }
}

#[cfg(test)]
mod tests;
