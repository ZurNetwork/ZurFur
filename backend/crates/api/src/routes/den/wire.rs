use application::den::{self, DenAccess, DenContent, DenSegment, node, root};

use crate::generated::{
    DenCard, DenCrumb, DenFile, DenListing, DenNode, DenOpen, GetDenNodeResponse,
    GetDenRootResponse, den_node, get_den_node_response, get_den_root_response,
};

/// A Den path as the wire carries it: each piece as the application answered it.
fn segments(segments: Vec<DenSegment>) -> Vec<String> {
    segments
        .iter()
        .map(|segment| segment.as_ref().to_owned())
        .collect()
}

impl From<den::DenNode> for DenNode {
    /// An open node carries its kind and its `open` access; a card carries neither kind nor
    /// anything beyond its entry.
    fn from(node: den::DenNode) -> Self {
        let (kind, access) = match node.access {
            DenAccess::Open {
                kind,
                own_level,
                content_not_shown,
                soft_delete,
            } => {
                let open = DenOpen {
                    own_level: own_level.to_string(),
                    content_not_shown,
                    soft_delete: soft_delete.map(|word| word.to_string()).unwrap_or_default(),
                };
                (kind.to_string(), den_node::Access::Open(open))
            }
            DenAccess::Card => (String::new(), den_node::Access::Card(DenCard {})),
        };
        DenNode {
            segments: segments(node.segments),
            name: node.name.to_string(),
            r#type: node.den_type.to_string(),
            kind,
            mount: node.mount,
            access: Some(access),
        }
    }
}

impl From<den::DenCrumb> for DenCrumb {
    fn from(crumb: den::DenCrumb) -> Self {
        DenCrumb {
            segments: segments(crumb.segments),
            name: crumb.name.to_string(),
        }
    }
}

impl From<den::DenListing> for DenListing {
    /// The last page carries no next-page token.
    fn from(listing: den::DenListing) -> Self {
        DenListing {
            entries: listing.entries.into_iter().map(DenNode::from).collect(),
            next_page_token: listing
                .next_page
                .map(|token| token.to_string())
                .unwrap_or_default(),
        }
    }
}

impl From<root::Output> for GetDenRootResponse {
    fn from(output: root::Output) -> Self {
        let listing = DenListing::from(output.listing);
        GetDenRootResponse {
            node: Some(DenNode::from(output.node)),
            content: Some(get_den_root_response::Content::Listing(listing)),
        }
    }
}

impl From<DenContent> for get_den_node_response::Content {
    fn from(content: DenContent) -> Self {
        match content {
            DenContent::Listing(listing) => Self::Listing(DenListing::from(listing)),
            DenContent::File => Self::File(DenFile {}),
        }
    }
}

impl From<node::Output> for GetDenNodeResponse {
    /// A card's answer carries no content.
    fn from(output: node::Output) -> Self {
        GetDenNodeResponse {
            node: Some(DenNode::from(output.node)),
            crumbs: output.crumbs.into_iter().map(DenCrumb::from).collect(),
            content: output.content.map(get_den_node_response::Content::from),
        }
    }
}

#[cfg(test)]
mod tests;
