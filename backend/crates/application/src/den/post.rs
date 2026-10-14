//! Den's arm for a poster's posts, `/post/<did>`: an empty, always-public stub that answers
//! the same for every DID and reads no store and no other host.

use domain::elements::did::Did;

use crate::den::{
    DenContext, DenMount, DenType, Level, Node, NodeState, Place, Segment, TopLevel,
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
            PostPlace::Posts(_) => DenType::Post,
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
/// no lifts, and live, for any DID and any viewer, with no store call.
pub(crate) async fn node(_context: &DenContext<'_>, poster: &Did) -> anyhow::Result<Option<Node>> {
    let Some(name) = names::fixed_name(POSTS) else {
        return Ok(None);
    };
    let node = Node {
        place: Place::Post(PostPlace::Posts(poster.clone())),
        name,
        level: Level::Public,
        lifts: Vec::new(),
        state: NodeState::Live,
    };
    Ok(Some(node))
}

/// The node at `key` in `place`: always absent, since the stub holds nothing.
pub(crate) async fn child(
    _context: &DenContext<'_>,
    _place: &PostPlace,
    _key: &Segment,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// Every real child of `place`: always empty, since the stub holds nothing.
pub(crate) async fn children(
    _context: &DenContext<'_>,
    _place: &PostPlace,
) -> anyhow::Result<Vec<Node>> {
    Ok(Vec::new())
}

/// The viewer's mounts in `place`: always empty, since the stub holds nothing.
pub(crate) async fn mounts(
    _context: &DenContext<'_>,
    _place: &PostPlace,
) -> anyhow::Result<Vec<DenMount>> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests;
