use atta::{Access, Admission, Level, Reach, Via};

use crate::den::NodeState;

/// Whether to keep a node, from its recorded state alone: an unrecorded node is refused, a
/// live one admitted, and a soft-deleted one admitted only to a viewer with their own
/// relationship to it, and in a listing only when deleted nodes are asked for.
pub(crate) fn admission(
    state: Option<NodeState>,
    access: Access,
    reach: Reach,
    include_deleted: bool,
) -> Admission {
    let Some(state) = state else {
        return Admission::Refuse;
    };
    match state {
        NodeState::Live => Admission::Admit,
        NodeState::Archived => archived(access, reach, include_deleted),
        NodeState::Deactivated { viewer_is_owner } => {
            if viewer_is_owner {
                soft_deleted_reach(reach, include_deleted)
            } else {
                Admission::Refuse
            }
        }
    }
}

/// An archived commission: refused as a card and when only its own level opens it; otherwise
/// kept as a soft-deleted node.
fn archived(access: Access, reach: Reach, include_deleted: bool) -> Admission {
    let opened = match access.level {
        Level::Private | Level::Listed => false,
        Level::Public => true,
    };
    let own_relationship = match access.via {
        Via::OwnLevel => false,
        Via::Standing | Via::Grant => true,
    };
    if opened && own_relationship {
        soft_deleted_reach(reach, include_deleted)
    } else {
        Admission::Refuse
    }
}

/// A soft-deleted node the viewer may open: kept on a path, and in a listing only when
/// deleted nodes are asked for.
fn soft_deleted_reach(reach: Reach, include_deleted: bool) -> Admission {
    match reach {
        Reach::Path => Admission::Admit,
        Reach::Listing if include_deleted => Admission::Admit,
        Reach::Listing => Admission::Refuse,
    }
}

#[cfg(test)]
mod tests;
