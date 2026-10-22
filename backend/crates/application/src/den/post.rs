//! Den's arm for a poster's posts, `/post/<did>`: an empty, always-public stub that answers
//! the same for every DID. It takes no ports, so it can read no store and call no host.

use domain::elements::{did::Did, level::Level};

use crate::den::{
    DenMount, DenType, Node, NodeState, Place, Segment, TopLevel,
    names::{self, POSTS},
};

/// The places this arm answers for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PostPlace {
    /// A poster's posts, `/post/<did>`.
    Posts(Did),
}

impl PostPlace {
    /// The type of the node at this place.
    pub(crate) fn den_type(&self) -> DenType {
        match self {
            PostPlace::Posts(_) => DenType::Posts,
        }
    }

    /// The node's segment in its real parent.
    pub(crate) fn key(&self) -> Option<Segment> {
        match self {
            PostPlace::Posts(poster) => names::did_segment(poster),
        }
    }

    /// The top-level directory the node sits directly under, if it does.
    pub(crate) fn top_level(&self) -> Option<TopLevel> {
        match self {
            PostPlace::Posts(_) => Some(TopLevel::Post),
        }
    }
}

/// The posts of `poster` at `/post/<poster>`: named by the fixed word `posts`, Public, with
/// no lifts, and live, for any DID and any viewer.
pub(crate) fn node(poster: &Did) -> Option<Node> {
    let name = names::fixed_name(POSTS)?;
    let node = Node {
        place: Place::Post(PostPlace::Posts(poster.clone())),
        name,
        level: Level::Public,
        lifts: Vec::new(),
        state: NodeState::Live,
    };
    Some(node)
}

/// The node at `key` in `place`: always absent, since the stub holds nothing.
pub(crate) fn child(_place: &PostPlace, _key: &Segment) -> Option<Node> {
    None
}

/// Every real child of `place`: always empty, since the stub holds nothing.
pub(crate) fn children(_place: &PostPlace) -> Vec<Node> {
    Vec::new()
}

/// The viewer's mounts in `place`: always empty, since the stub holds nothing.
pub(crate) fn mounts(_place: &PostPlace) -> Vec<DenMount> {
    Vec::new()
}

#[cfg(test)]
mod tests;
