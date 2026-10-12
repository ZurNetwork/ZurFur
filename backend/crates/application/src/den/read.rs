use std::str::FromStr;

use atta::{Caps, Entry, Kind, Level, RealPath, Resolved, Resolver, Segment, View};
use domain::elements::user::UserId;

use crate::den::{
    DenAccess, DenContent, DenCrumb, DenError, DenListing, DenName, DenNode, DenSegment, DenType,
    NodeKind, NodeLevel, PageToken, TopLevel, host::DenHost, names,
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

/// One crumb above a node.
pub(crate) fn crumb(entry: &Entry) -> DenCrumb {
    DenCrumb {
        segments: segments(&entry.view),
        name: DenName::new(entry.name.clone()),
    }
}

/// A Den read for one request: the engine's resolver over Zurfur's host.
pub(crate) struct Reader<'h> {
    host: &'h DenHost<'h>,
    resolver: Resolver<'h>,
}

impl<'h> Reader<'h> {
    /// A reader over `host`, for the viewer it was built for, with the default caps.
    pub(crate) fn new(host: &'h DenHost<'h>) -> Self {
        let resolver = Resolver {
            host,
            viewer: host.viewer().clone(),
            caps: Caps::default(),
        };
        Self { host, resolver }
    }

    /// The node at `den_path` below `root`, with its crumbs.
    pub(crate) async fn walk(
        &self,
        root: &RealPath,
        den_path: &[Segment],
    ) -> Result<Resolved, DenError> {
        // Called by path, not as a method: the network guard reserves the method-call spelling
        // for an HTTP client's DNS override, and this call reaches no network.
        Resolver::resolve(&self.resolver, root, den_path)
            .await
            .map_err(engine_error)
    }

    /// One entry as the Den shows it; an open entry is marked with its soft-deleted word, read
    /// from the host's record for its real path.
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
                let soft_delete = self.host.soft_delete(real_path).ok_or_else(|| {
                    let unrecorded = anyhow::anyhow!("an open den node has no host record");
                    DenError::Store(unrecorded)
                })?;
                DenAccess::Open {
                    kind: node_kind(*kind),
                    own_level: node_level(*own_level),
                    content_not_shown: den_type.content_not_shown(),
                    soft_delete,
                }
            }
        };
        let node = DenNode {
            segments: segments(&entry.view),
            name: DenName::new(entry.name.clone()),
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
        if den_type(resolved.node())?.content_not_shown() {
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
