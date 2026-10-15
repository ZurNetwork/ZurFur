//! Identity's arm of the Den for a Character's directory, `/character/<did>`.

use domain::elements::character::{CharacterId, Presence};

use crate::den::{
    DenContext, DenMount, DenType, Level, Node, NodeState, Place, STANDING, Segment, TopLevel,
    names,
};

/// The places in a Character's directory that this arm answers for.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// The Character at `/character/<character>`, answered from the Characters the viewer keeps,
/// the same read whatever the DID: kept, it opens by the keeper's standing at its own level;
/// otherwise it is absent. Nothing mounts another's Character yet, so one the viewer doesn't
/// keep is never reached; the slice that adds shared Characters needs a read by DID.
pub(crate) async fn node(
    context: &DenContext<'_>,
    character: &CharacterId,
) -> anyhow::Result<Option<Node>> {
    let kept = context
        .ports
        .characters
        .list_kept_by(context.viewer)
        .await?;
    let Some(kept) = kept.into_iter().find(|kept| &kept.id == character) else {
        return Ok(None);
    };
    let Some(name) = names::stored_name(kept.attributes.name.as_ref()) else {
        return Ok(None);
    };
    let node = Node {
        place: Place::Character(CharacterPlace::Character(kept.id)),
        name,
        level: presence_level(&kept.presence),
        lifts: vec![STANDING],
        state: NodeState::Live,
    };
    Ok(Some(node))
}

/// A Character's own level, from its stored presence.
fn presence_level(presence: &Presence) -> Level {
    match presence {
        Presence::Private => Level::Private,
        Presence::Public { .. } => Level::Public,
    }
}

/// The node at `key` in `place`: none, since a Character's art isn't served yet.
pub(crate) async fn child(
    _context: &DenContext<'_>,
    _place: &CharacterPlace,
    _key: &Segment,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// Every real child of `place`: none, since a Character's art isn't served yet.
pub(crate) async fn children(
    _context: &DenContext<'_>,
    _place: &CharacterPlace,
) -> anyhow::Result<Vec<Node>> {
    Ok(Vec::new())
}

/// The viewer's mounts in `place`: none.
pub(crate) async fn mounts(
    _context: &DenContext<'_>,
    _place: &CharacterPlace,
) -> anyhow::Result<Vec<DenMount>> {
    Ok(Vec::new())
}

#[cfg(test)]
mod tests;
