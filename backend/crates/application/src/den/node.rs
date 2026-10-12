//! Reads the node at a Den path below the viewer's own root.

use atta::Caps;
use domain::elements::user::UserId;

use crate::{
    Ports,
    den::{
        Den, DenContent, DenCrumb, DenError, DenNode, PageToken,
        host::DenHost,
        read::{Reader, crumb, den_path, viewer_root},
    },
    use_case,
};

/// Reads the node at a Den path below the session viewer's own root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeQuery {
    /// The viewer, from the session only.
    pub viewer: UserId,
    /// The Den path's pieces, already split on `/` and percent-decoded once by the driver.
    pub path: Vec<String>,
    /// Whether listings also show soft-deleted nodes the viewer may open.
    pub include_deleted: bool,
    /// Where a listing starts; `None` for the first page.
    pub page: Option<PageToken>,
}

/// The node at the path, its crumbs, and what it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// The node the path lands on.
    pub node: DenNode,
    /// The folders above it, the root first, down to its parent.
    pub crumbs: Vec<DenCrumb>,
    /// What it holds; `None` on a card.
    pub content: Option<DenContent>,
}

impl Den<'_> {
    /// The node at `query.path` below the viewer's own root. An empty path, more than 16
    /// pieces, or a piece that isn't a segment is the one not-found, before any store is read.
    #[use_case]
    pub async fn node(&self, #[ports] ports: &Ports, query: NodeQuery) -> Result<Output, DenError> {
        let segments = den_path(&query.path, Caps::default())?;
        let root = viewer_root(&query.viewer)?;
        let host = DenHost::new(ports, &query.viewer, query.include_deleted);
        let reader = Reader::new(&host);
        let resolved = reader.walk(&root, &segments).await?;
        let node = reader.node(resolved.node())?;
        let crumbs = resolved.crumbs().iter().map(crumb).collect();
        let content = reader.content(&resolved, query.page).await?;
        let output = Output {
            node,
            crumbs,
            content,
        };
        Ok(output)
    }
}
