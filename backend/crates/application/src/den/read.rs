//! The Den read's shared steps: the viewer's root, the Den path, and the node, listing and
//! crumbs the use cases answer with.

use std::str::FromStr;

use atta::{Caps, Entry, Host, Kind, Level, RealPath, Resolved, Resolver, Segment, View, Viewer};
use domain::elements::user::UserId;

use crate::den::{
    DenAccess, DenContent, DenCrumb, DenError, DenListing, DenName, DenNode, DenSegment, DenThing,
    DenType, NameOrdinal, NodeKind, NodeLevel, PageToken, SoftDelete, TopLevel, host::DenHost,
    names,
};

/// The engine's read error as the Den's: every miss is the one not-found.
fn engine_error(error: atta::Error) -> DenError {
    match error {
        atta::Error::NotFound => DenError::NotFound,
        atta::Error::Host(host_error) => DenError::Store(anyhow::Error::from(host_error)),
    }
}

/// The viewer's own root, `/user/<their DID>`, built from the session's user alone.
pub(crate) fn viewer_root(viewer: &UserId) -> Result<RealPath, DenError> {
    let top_level = names::fixed_segment(TopLevel::User.into()).ok_or(DenError::NotFound)?;
    let key = Segment::try_from(viewer.as_ref()).map_err(|_| DenError::NotFound)?;
    let root = RealPath::from(vec![top_level, key]);
    Ok(root)
}

/// The pieces a driver split and decoded, as a Den path below the root. An empty path, one
/// longer than `caps` allows, or any piece that isn't a segment is the one not-found.
pub(crate) fn den_path(pieces: &[String], caps: Caps) -> Result<Vec<Segment>, DenError> {
    if pieces.is_empty() || pieces.len() > caps.max_den_path {
        return Err(DenError::NotFound);
    }
    pieces
        .iter()
        .map(|piece| Segment::try_from(piece.as_str()).map_err(|_| DenError::NotFound))
        .collect()
}

/// The Den type an entry carries; a type the host never answers with is a broken host.
fn den_type(entry: &Entry) -> Result<DenType, DenError> {
    DenType::from_str(entry.typ.as_ref()).map_err(|_| {
        let unknown_type = anyhow::anyhow!("the den host answered with a type it doesn't know");
        DenError::Store(unknown_type)
    })
}

/// A level in the Den's own words.
fn node_level(level: Level) -> NodeLevel {
    match level {
        Level::Private => NodeLevel::Private,
        Level::Listed => NodeLevel::Listed,
        Level::Public => NodeLevel::Public,
    }
}

/// A kind in the Den's own words.
fn node_kind(kind: Kind) -> NodeKind {
    match kind {
        Kind::Directory => NodeKind::Directory,
        Kind::File => NodeKind::File,
    }
}

/// The Den path an entry shows: its own when the viewer can open it, else empty.
fn segments(view: &View) -> Vec<DenSegment> {
    match view {
        View::Open { den_path, .. } => den_path.iter().cloned().map(DenSegment::new).collect(),
        View::Card => Vec::new(),
    }
}

/// One crumb above a node, with the type its entry carries; a type the host never answers
/// with is a broken host.
pub(crate) fn crumb(entry: &Entry) -> Result<DenCrumb, DenError> {
    let crumb = DenCrumb {
        segments: segments(&entry.view),
        name: DenName::new(entry.name.clone()),
        name_ordinal: entry.ordinal.map(NameOrdinal::new),
        den_type: den_type(entry)?,
    };
    Ok(crumb)
}

/// What the host recorded about a node it answered for: its soft-deleted word and the thing
/// it stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RecordedFacts {
    /// Its soft-deleted word; `None` when live.
    pub(crate) soft_delete: Option<SoftDelete>,
    /// The thing it stands for; `None` for a node that stands for none.
    pub(crate) thing: Option<DenThing>,
}

/// Where a Den read finds what the host recorded about a node it answered for; `Sync`, so a
/// read's future can cross threads.
pub(crate) trait RecordLookup: Sync {
    /// The facts recorded for the node at `path`; `None` when the host never answered for
    /// that path.
    fn recorded(&self, path: &RealPath) -> Option<RecordedFacts>;
}

/// A Den read for one request: the engine's resolver over a host, and where the host keeps
/// its record.
pub(crate) struct Reader<'h> {
    resolver: Resolver<'h>,
    records: &'h dyn RecordLookup,
}

impl<'h> Reader<'h> {
    /// A reader over any `host` for `viewer` within `caps`, marking open nodes from
    /// `records`; the den Queries use [`Reader::over`].
    pub(crate) fn new(
        host: &'h dyn Host,
        viewer: Viewer,
        records: &'h dyn RecordLookup,
        caps: Caps,
    ) -> Self {
        let resolver = Resolver { host, viewer, caps };
        Self { resolver, records }
    }

    /// A reader over Zurfur's host for the viewer it was built for, marking open nodes from
    /// that same host's record, within `caps`.
    pub(crate) fn over(host: &'h DenHost<'h>, caps: Caps) -> Self {
        Self::new(host, host.viewer().clone(), host, caps)
    }

    /// The node at `den_path` below `root`, with its crumbs.
    pub(crate) async fn walk(
        &self,
        root: &RealPath,
        den_path: &[Segment],
    ) -> Result<Resolved, DenError> {
        self.resolver
            .resolve_path(root, den_path)
            .await
            .map_err(engine_error)
    }

    /// The walked node and its crumbs, each numbered among its twins in the listing of the
    /// folder it was reached through. Lists each folder on the path once more.
    pub(crate) async fn tell_apart(&self, resolved: Resolved) -> Result<Resolved, DenError> {
        self.resolver
            .tell_apart(resolved)
            .await
            .map_err(engine_error)
    }

    /// One entry as the Den shows it; an open entry is marked with its soft-deleted word and
    /// the thing it stands for, read from the host's record for its real path.
    pub(crate) fn node(&self, entry: &Entry) -> Result<DenNode, DenError> {
        let den_type = den_type(entry)?;
        let access = match &entry.view {
            View::Card => DenAccess::Card,
            View::Open {
                real_path,
                own_level,
                kind,
                ..
            } => {
                let RecordedFacts { soft_delete, thing } =
                    self.records.recorded(real_path).ok_or_else(|| {
                        let unrecorded = anyhow::anyhow!("an open den node has no host record");
                        DenError::Store(unrecorded)
                    })?;
                DenAccess::Open {
                    kind: node_kind(*kind),
                    own_level: node_level(*own_level),
                    content_not_shown: den_type.content_not_shown(),
                    soft_delete,
                    thing,
                }
            }
        };
        let node = DenNode {
            segments: segments(&entry.view),
            name: DenName::new(entry.name.clone()),
            name_ordinal: entry.ordinal.map(NameOrdinal::new),
            den_type,
            mount: entry.mount,
            access,
        };
        Ok(node)
    }

    /// What the resolved node holds: `None` on a card, a file's empty content, or one page of
    /// an open directory. A directory whose type doesn't show its content yet answers an empty
    /// page without reading its children.
    pub(crate) async fn content(
        &self,
        resolved: &Resolved,
        page: Option<PageToken>,
    ) -> Result<Option<DenContent>, DenError> {
        let entry = resolved.node();
        let View::Open { kind, .. } = &entry.view else {
            return Ok(None);
        };
        match kind {
            Kind::File => Ok(Some(DenContent::File)),
            Kind::Directory => {
                let listing = self.listing(resolved, page).await?;
                Ok(Some(DenContent::Listing(listing)))
            }
        }
    }

    /// One page of the resolved open directory.
    pub(crate) async fn listing(
        &self,
        resolved: &Resolved,
        page: Option<PageToken>,
    ) -> Result<DenListing, DenError> {
        let node = resolved.node();
        let View::Open { .. } = &node.view else {
            return Err(DenError::NotFound);
        };
        if den_type(node)?.content_not_shown() {
            return Ok(DenListing::default());
        }
        let page = page.map(PageToken::into_inner);
        let listing = self
            .resolver
            .list(resolved, page)
            .await
            .map_err(engine_error)?
            .ok_or(DenError::NotFound)?;
        let entries = listing
            .entries
            .iter()
            .map(|entry| self.node(entry))
            .collect::<Result<_, _>>()?;
        let listing = DenListing {
            entries,
            next_page: listing.next.map(PageToken::new),
        };
        Ok(listing)
    }
}

#[cfg(test)]
mod tests;
