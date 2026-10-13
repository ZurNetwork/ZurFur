//! Den's arm for a poster's posts, `/post/<did>`.

use domain::elements::did::Did;

use crate::den::{DenContext, DenMount, DenType, Node, Segment, TopLevel, names};

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

/// The posts of `poster` at `/post/<poster>`, for the session's viewer. Not served yet:
/// always absent.
pub(crate) async fn node(_context: &DenContext<'_>, _poster: &Did) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// The node at `key` in `place`. Not served yet: always absent.
pub(crate) async fn child(
    _context: &DenContext<'_>,
    _place: &PostPlace,
    _key: &Segment,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// Every real child of `place`. Not served yet: always empty.
pub(crate) async fn children(
    _context: &DenContext<'_>,
    _place: &PostPlace,
) -> anyhow::Result<Vec<Node>> {
    Ok(Vec::new())
}

/// The viewer's mounts in `place`. Not served yet: always empty.
pub(crate) async fn mounts(
    _context: &DenContext<'_>,
    _place: &PostPlace,
) -> anyhow::Result<Vec<DenMount>> {
    Ok(Vec::new())
}
