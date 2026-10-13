//! Identity's arm of the Den for a Character's directory, `/character/<did>`.

use domain::elements::character::CharacterId;

use crate::den::{DenContext, DenMount, DenType, Node, Segment, TopLevel, names};

/// The places in a Character's directory that this arm answers for.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "the identity arms build these once they serve a Character's nodes"
)]
pub(crate) enum CharacterPlace {
    /// The Character's directory, `/character/<did>`.
    Character(CharacterId),
}

impl CharacterPlace {
    /// The type of the node at this place.
    pub(crate) fn den_type(&self) -> DenType {
        match self {
            CharacterPlace::Character(_) => DenType::Character,
        }
    }

    /// The node's segment in its real parent.
    pub(crate) fn key(&self) -> Option<Segment> {
        match self {
            CharacterPlace::Character(character) => names::did_segment(character.did()),
        }
    }

    /// The top-level directory the node sits directly under, if it does.
    pub(crate) fn top_level(&self) -> Option<TopLevel> {
        match self {
            CharacterPlace::Character(_) => Some(TopLevel::Character),
        }
    }
}

/// The Character at `/character/<character>`, for the session's viewer. Not served yet:
/// always absent.
pub(crate) async fn node(
    _context: &DenContext<'_>,
    _character: &CharacterId,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// The node at `key` in `place`. Not served yet: always absent.
pub(crate) async fn child(
    _context: &DenContext<'_>,
    _place: &CharacterPlace,
    _key: &Segment,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// Every real child of `place`. Not served yet: always empty.
pub(crate) async fn children(
    _context: &DenContext<'_>,
    _place: &CharacterPlace,
) -> anyhow::Result<Vec<Node>> {
    Ok(Vec::new())
}

/// The viewer's mounts in `place`. Not served yet: always empty.
pub(crate) async fn mounts(
    _context: &DenContext<'_>,
    _place: &CharacterPlace,
) -> anyhow::Result<Vec<DenMount>> {
    Ok(Vec::new())
}
