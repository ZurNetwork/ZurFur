use application::den::{self, DenAccess, DenContent, DenSegment, DenThing, node, root};

use crate::generated::{
    DenCard, DenCrumb, DenFile, DenListing, DenNode, DenOpen, GetDenNodeResponse,
    GetDenRootResponse, den_node, den_open, get_den_node_response, get_den_root_response,
};

/// A Den path as the wire carries it: each piece as the application answered it.
fn segments(segments: Vec<DenSegment>) -> Vec<String> {
    segments
        .iter()
        .map(|segment| segment.as_ref().to_owned())
        .collect()
}

impl From<DenThing> for den_open::Thing {
    /// An Account as its DID, a commission as its id.
    fn from(thing: DenThing) -> Self {
        match thing {
            DenThing::Account(account) => Self::AccountDid(account.to_string()),
            DenThing::Commission(commission) => Self::CommissionId(commission.to_string()),
        }
    }
}

impl From<den::DenNode> for DenNode {
    /// An open node carries its kind and its `open` access, with the thing it stands for; a
    /// card carries neither kind nor anything beyond its entry. Either carries the number
    /// that tells its name apart, when it has one.
    fn from(node: den::DenNode) -> Self {
        let (kind, access) = match node.access {
            DenAccess::Open {
                kind,
                own_level,
                content_not_shown,
                soft_delete,
                thing,
            } => {
                let open = DenOpen {
                    own_level: own_level.to_string(),
                    content_not_shown,
                    soft_delete: soft_delete.map(|word| word.to_string()).unwrap_or_default(),
                    thing: thing.map(den_open::Thing::from),
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
            name_ordinal: node.name_ordinal.map(u32::from),
            access: Some(access),
        }
    }
}

impl From<den::DenCrumb> for DenCrumb {
    /// A crumb carries its type's word for its icon, and the number its parent's listing
    /// gives its name.
    fn from(crumb: den::DenCrumb) -> Self {
        DenCrumb {
            segments: segments(crumb.segments),
            name: crumb.name.to_string(),
            r#type: crumb.den_type.to_string(),
            name_ordinal: crumb.name_ordinal.map(u32::from),
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
