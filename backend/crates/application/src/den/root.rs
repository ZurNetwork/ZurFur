//! Reads the viewer's own root.

use atta::Caps;
use domain::elements::user::UserId;

use crate::{
    Ports,
    den::{
        Den, DenError, DenListing, DenNode, PageToken,
        host::DenHost,
        read::{Reader, viewer_root},
    },
    use_case,
};

/// Reads the session viewer's own root; nothing in it names a root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootQuery {
    /// The viewer, from the session only.
    pub viewer: UserId,
    /// Whether listings also show soft-deleted nodes the viewer may open.
    pub include_deleted: bool,
    /// Where the listing starts; `None` for the first page.
    pub page: Option<PageToken>,
}

/// The viewer's root and one page of its entries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    /// The root itself: no segments, its own level private.
    pub node: DenNode,
    /// One page of what it holds.
    pub listing: DenListing,
}

impl Den<'_> {
    /// The viewer's own root, `/user/<their DID>`, and one page of its entries.
    #[use_case]
    pub async fn root(&self, #[ports] ports: &Ports, query: RootQuery) -> Result<Output, DenError> {
        let root = viewer_root(&query.viewer)?;
        let host = DenHost::new(ports, &query.viewer, query.include_deleted);
        let reader = Reader::over(&host, Caps::default());
        let resolved = reader.walk(&root, &[]).await?;
        let node = reader.node(resolved.node())?;
        let listing = reader.listing(&resolved, query.page).await?;
        let output = Output { node, listing };
        Ok(output)
    }
}
