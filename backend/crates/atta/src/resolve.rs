use crate::{
    Access, Admission, Found, Host, HostError, Kind, Level, Lift, NodeName, Reach, RealPath,
    Relationship, Scope, Segment, Type, Via, Viewer,
};

/// The read caps the host sets for one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caps {
    /// The most segments a Den path may hold.
    pub max_den_path: usize,
    /// The most entries one page of a listing holds; at least one is always served.
    pub page_size: usize,
}

impl Default for Caps {
    /// 16 segments and 50 entries a page.
    fn default() -> Self {
        Self {
            max_den_path: 16,
            page_size: 50,
        }
    }
}

/// Why a read failed. Every miss is the one [`Error::NotFound`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The node is absent, doesn't exist, sits inside a node the viewer can't open, or was refused.
    #[error("node not found")]
    NotFound,
    /// The host failed.
    #[error(transparent)]
    Host(#[from] HostError),
}

/// What the viewer sees of a node they can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum View {
    /// They can open it, and they see its own level.
    Open {
        /// The node's own level; a mount shows its target's.
        own_level: Level,
    },
    /// They see a card: they may know it exists, but can't open it.
    Card,
}

/// One node as the viewer sees it, at a Den path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Its Den path from the viewer's root; empty for the root.
    pub den_path: Vec<Segment>,
    /// Where it really lives; never for the viewer's eyes.
    pub real_path: RealPath,
    /// Its display name; a mount shows its target's.
    pub name: NodeName,
    /// Its type; a mount shows its target's.
    pub typ: Type,
    /// Whether it is a directory or a file; a mount shows its target's.
    pub kind: Kind,
    /// Whether it is one of the viewer's mounts rather than a real child of its folder.
    pub mount: bool,
    /// What the viewer sees of it.
    pub view: View,
}

/// A typed Den path, resolved for the viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    /// The folders above the node, the viewer's root first, down to its parent.
    pub crumbs: Vec<Entry>,
    /// The node the path lands on.
    pub node: Entry,
}

/// One node Atta has located on a real path, with what the viewer holds there.
#[derive(Debug, Clone)]
pub(crate) struct Located {
    pub(crate) found: Found,
    pub(crate) path: RealPath,
    pub(crate) access: Access,
    pub(crate) below: Vec<Lift>,
}

/// Reads the tree for one viewer, through the host.
pub struct Resolver<'h> {
    /// The application Atta asks about nodes.
    pub host: &'h dyn Host,
    /// Who is reading.
    pub viewer: Viewer,
    /// The read caps.
    pub caps: Caps,
}

/// The viewer's access to a node from its own level, its lifts and the subtree lifts above it.
///
/// The highest level wins. On a tie, a relationship of the viewer's own wins over the node's
/// own level, and standing wins over a grant.
pub(crate) fn access(own_level: Level, here: &[Lift], inherited: &[Lift]) -> Access {
    let lifts = || here.iter().chain(inherited);
    let lifted = lifts()
        .map(|lift| lift.level)
        .max()
        .unwrap_or(Level::Private);
    let level = own_level.max(lifted);
    let reaching = |relationship: Relationship| {
        lifts().any(|lift| lift.level == level && lift.relationship == relationship)
    };
    let via = if reaching(Relationship::Standing) {
        Via::Standing
    } else if reaching(Relationship::Grant) {
        Via::Grant
    } else {
        Via::OwnLevel
    };
    Access { level, via }
}

impl Resolver<'_> {
    /// The node at `den_path` below the viewer's `root`, with its crumbs; any miss is
    /// [`Error::NotFound`].
    pub async fn resolve(&self, root: &RealPath, den_path: &[Segment]) -> Result<Resolved, Error> {
        if den_path.len() > self.caps.max_den_path {
            return Err(Error::NotFound);
        }
        let mut current = self.locate(root).await?.ok_or(Error::NotFound)?;
        let mut crumbs = Vec::new();
        let mut mount = false;
        let mut walked = Vec::new();
        for segment in den_path {
            self.enter(&current, false).await?;
            crumbs.push(self.entry(&current, walked.clone(), mount));
            let (next, next_mount) = self.step(&current, segment).await?;
            walked.push(segment.clone());
            current = next;
            mount = next_mount;
        }
        self.enter(&current, true).await?;
        let node = self.entry(&current, walked, mount);
        let resolved = Resolved { crumbs, node };
        Ok(resolved)
    }

    /// Checks the viewer may stand on `located` as a step of a typed path, `last` or not.
    async fn enter(&self, located: &Located, last: bool) -> Result<(), Error> {
        if located.access.level == Level::Private {
            return Err(Error::NotFound);
        }
        let passes_through =
            located.access.level == Level::Public && self.kind(located) == Kind::Directory;
        if !last && !passes_through {
            return Err(Error::NotFound);
        }
        let admission = self
            .host
            .admit(
                &self.viewer,
                &located.path,
                &located.found.metadata,
                located.access,
                Reach::Path,
            )
            .await?;
        if admission == Admission::Refuse {
            return Err(Error::NotFound);
        }
        Ok(())
    }

    /// The node at `segment` in the open directory `folder`: its real child or one of the
    /// viewer's mounts, never both; the flag is set for a mount.
    async fn step(&self, folder: &Located, segment: &Segment) -> Result<(Located, bool), Error> {
        let child = self.host.child(&self.viewer, &folder.path, segment).await?;
        let mounts = self.host.mounts(&self.viewer, &folder.path).await?;
        let mut matching = mounts.into_iter().filter(|mount| &mount.segment == segment);
        let mount = matching.next();
        if matching.next().is_some() {
            return Err(Error::NotFound);
        }
        match (child, mount) {
            (Some(found), None) => {
                let located = self
                    .check(&folder.path, segment, &folder.below, found)
                    .ok_or(Error::NotFound)?;
                Ok((located, false))
            }
            (None, Some(mount)) => {
                let target = self.locate(&mount.target).await?.ok_or(Error::NotFound)?;
                Ok((target, true))
            }
            (Some(_), Some(_)) | (None, None) => Err(Error::NotFound),
        }
    }

    /// The node at `path`, which must sit directly under a top-level directory, as a viewer's
    /// root and every mount target do; `None` at any other depth. The top-level directory opens
    /// by rule but must be a directory.
    pub(crate) async fn locate(&self, path: &RealPath) -> Result<Option<Located>, Error> {
        let [top_level, key] = path.segments() else {
            return Ok(None);
        };
        let root = RealPath::default();
        let Some(found) = self.host.child(&self.viewer, &root, top_level).await? else {
            return Ok(None);
        };
        let Some(directory) = self.check(&root, top_level, &[], found) else {
            return Ok(None);
        };
        if self.kind(&directory) != Kind::Directory {
            return Ok(None);
        }
        let found = self.host.child(&self.viewer, &directory.path, key).await?;
        let located =
            found.and_then(|found| self.check(&directory.path, key, &directory.below, found));
        Ok(located)
    }

    /// Locates a node the host answered for at `key` in `parent`, refusing an answer that
    /// names another place, and any symlink: links resolve nowhere yet.
    pub(crate) fn check(
        &self,
        parent: &RealPath,
        key: &Segment,
        inherited: &[Lift],
        found: Found,
    ) -> Option<Located> {
        let metadata = &found.metadata;
        let elsewhere = &metadata.key != key || metadata.parent.as_ref() != Some(parent);
        if elsewhere || metadata.symlink_target.is_some() {
            return None;
        }
        let access = access(metadata.level, &found.lifts, inherited);
        let subtree_here = found
            .lifts
            .iter()
            .filter(|lift| lift.scope == Scope::Subtree);
        let below = inherited.iter().chain(subtree_here).copied().collect();
        let path = parent.join(key.clone());
        let located = Located {
            found,
            path,
            access,
            below,
        };
        Some(located)
    }

    /// Whether a located node is a directory or a file, as its type says.
    pub(crate) fn kind(&self, located: &Located) -> Kind {
        self.host.kind(&located.found.metadata.typ)
    }

    /// The entry the viewer sees for a located node at `den_path`.
    pub(crate) fn entry(&self, located: &Located, den_path: Vec<Segment>, mount: bool) -> Entry {
        let metadata = &located.found.metadata;
        let view = if located.access.level == Level::Public {
            View::Open {
                own_level: metadata.level,
            }
        } else {
            View::Card
        };
        Entry {
            den_path,
            real_path: located.path.clone(),
            name: metadata.name.clone(),
            typ: metadata.typ.clone(),
            kind: self.kind(located),
            mount,
            view,
        }
    }
}

#[cfg(test)]
mod tests;
