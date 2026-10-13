//! Commission's arm of the Den: the node at `/commission/<id>`, the entries inside it, and
//! the folders of commission mounts in a user's and an Account's directory.

use domain::elements::{
    account::AccountId,
    commission::{Commission, CommissionId, ElementId, FileKey, Visibility},
    user::UserId,
};

use crate::den::{
    DenContext, DenMount, DenType, Level, Node, NodeState, Place, STANDING, Segment, TopLevel,
    names::{self, ATTACHMENTS, CHANGELOG, COMMISSIONS, PRODUCTS, SLOTS},
};

/// The places this arm answers for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommissionPlace {
    /// A user's `commissions` folder, which holds a mount of each commission they are part of.
    #[allow(
        dead_code,
        reason = "built by the identity arms, which answer the user's folders"
    )]
    UserCommissions(UserId),
    /// An Account's `commissions` folder, which holds a mount of each commission brought into it.
    #[allow(
        dead_code,
        reason = "built by the identity arms, which answer the Account's folders"
    )]
    AccountCommissions(AccountId),
    /// The commission's directory, `/commission/<id>`.
    Commission(CommissionId),
    /// The commission's `attachments` folder.
    Attachments(CommissionId),
    /// The commission's `changelog` file.
    Changelog(CommissionId),
    /// The commission's `products` folder.
    Products(CommissionId),
    /// The commission's `slots` folder.
    Slots(CommissionId),
    /// One slot, `slots/<slot id>`.
    Slot(CommissionId, ElementId),
    /// One file in the commission, keyed by its file key.
    File(CommissionId, FileKey),
}

impl CommissionPlace {
    /// The type of the node at this place.
    pub(crate) fn den_type(&self) -> DenType {
        match self {
            CommissionPlace::UserCommissions(_) => DenType::UserCommissions,
            CommissionPlace::AccountCommissions(_) => DenType::AccountCommissions,
            CommissionPlace::Commission(_) => DenType::Commission,
            CommissionPlace::Attachments(_) => DenType::CommissionAttachments,
            CommissionPlace::Changelog(_) => DenType::CommissionChangelog,
            CommissionPlace::Products(_) => DenType::CommissionProducts,
            CommissionPlace::Slots(_) => DenType::CommissionSlots,
            CommissionPlace::Slot(..) => DenType::Slot,
            CommissionPlace::File(..) => DenType::File,
        }
    }

    /// The node's segment in its real parent.
    pub(crate) fn key(&self) -> Option<Segment> {
        match self {
            CommissionPlace::UserCommissions(_) | CommissionPlace::AccountCommissions(_) => {
                names::fixed_segment(COMMISSIONS)
            }
            CommissionPlace::Commission(commission) => names::id_segment(commission),
            CommissionPlace::Attachments(_) => names::fixed_segment(ATTACHMENTS),
            CommissionPlace::Changelog(_) => names::fixed_segment(CHANGELOG),
            CommissionPlace::Products(_) => names::fixed_segment(PRODUCTS),
            CommissionPlace::Slots(_) => names::fixed_segment(SLOTS),
            CommissionPlace::Slot(_, slot) => names::id_segment(slot),
            CommissionPlace::File(_, file) => names::id_segment(file),
        }
    }

    /// The top-level directory the node sits directly under, if it does.
    pub(crate) fn top_level(&self) -> Option<TopLevel> {
        match self {
            CommissionPlace::Commission(_) => Some(TopLevel::Commission),
            CommissionPlace::UserCommissions(_)
            | CommissionPlace::AccountCommissions(_)
            | CommissionPlace::Attachments(_)
            | CommissionPlace::Changelog(_)
            | CommissionPlace::Products(_)
            | CommissionPlace::Slots(_)
            | CommissionPlace::Slot(..)
            | CommissionPlace::File(..) => None,
        }
    }
}

/// A commission's own level: its stored visibility, one of the Den's three levels.
fn level_of(visibility: &Visibility) -> Level {
    match visibility {
        Visibility::Private => Level::Private,
        Visibility::Listed => Level::Listed,
        Visibility::Public => Level::Public,
    }
}

/// The node for a loaded commission: its visibility as its level, standing for a
/// Participant, archived or live, and its title through the name rule.
fn commission_node(commission: &Commission, participant: bool) -> Option<Node> {
    let name = names::stored_name(commission.title.as_str())?;
    let lifts = if participant {
        vec![STANDING]
    } else {
        Vec::new()
    };
    let state = if commission.is_archived() {
        NodeState::Archived
    } else {
        NodeState::Live
    };
    let node = Node {
        place: Place::Commission(CommissionPlace::Commission(commission.id)),
        name,
        level: level_of(&commission.visibility),
        lifts,
        state,
    };
    Some(node)
}

/// A Private, live entry with no lifts of its own, named `name`.
fn entry(place: CommissionPlace, name: names::Name) -> Option<Node> {
    let node = Node {
        place: Place::Commission(place),
        name: name.into_node_name()?,
        level: Level::Private,
        lifts: Vec::new(),
        state: NodeState::Live,
    };
    Some(node)
}

/// A commission's four fixed entries: the folders `attachments`, `products` and `slots`, and
/// the file `changelog`. Read from no store.
fn fixed_entries(commission: CommissionId) -> Vec<Node> {
    [
        (CommissionPlace::Attachments(commission), ATTACHMENTS),
        (CommissionPlace::Changelog(commission), CHANGELOG),
        (CommissionPlace::Products(commission), PRODUCTS),
        (CommissionPlace::Slots(commission), SLOTS),
    ]
    .into_iter()
    .filter_map(|(place, word)| entry(place, names::Name::Fixed(word)))
    .collect()
}

/// The commission's files, from its own file list; a file's name is read only for a key
/// in that list.
async fn attachments(
    context: &DenContext<'_>,
    commission: CommissionId,
    only: Option<&Segment>,
) -> anyhow::Result<Vec<Node>> {
    let files = context.ports.commissions.files(&commission).await?;
    let mut nodes = Vec::new();
    for file in files {
        let place = CommissionPlace::File(commission, file.id);
        if only.is_some_and(|key| names::id_segment(file.id).as_ref() != Some(key)) {
            continue;
        }
        let stored = context.ports.files.filename(file.id).await?;
        let name = stored.map_or(names::Name::Fixed(names::UNNAMED), names::Name::Stored);
        nodes.extend(entry(place, name));
    }
    Ok(nodes)
}

/// The commission's slots, from its own slot list, each named by its title.
async fn slots(
    context: &DenContext<'_>,
    commission: CommissionId,
    only: Option<&Segment>,
) -> anyhow::Result<Vec<Node>> {
    let slots = context.ports.commissions.slots(&commission).await?;
    let nodes = slots
        .into_iter()
        .filter(|slot| {
            only.is_none_or(|key| names::id_segment(slot.element_id).as_ref() == Some(key))
        })
        .filter_map(|slot| {
            let place = CommissionPlace::Slot(commission, slot.element_id);
            entry(place, names::Name::Stored(slot.title))
        })
        .collect();
    Ok(nodes)
}

/// The commission at `/commission/<commission>`, for the session's viewer. Always reads the
/// commission and then the viewer's participation, both, so an absent commission and one
/// that isn't the viewer's run the same statements.
pub(crate) async fn node(
    context: &DenContext<'_>,
    commission: CommissionId,
) -> anyhow::Result<Option<Node>> {
    let store = &context.ports.commissions;
    let found = store.find(&commission).await?;
    let participant = store.is_participant(&commission, context.viewer).await?;
    let node = found.and_then(|found| commission_node(&found, participant));
    Ok(node)
}

/// The node at `key` in the folder `place`: a fixed entry of a commission, a file from its
/// file list, or a slot from its slot list. The folders of mounts hold no real children.
pub(crate) async fn child(
    context: &DenContext<'_>,
    place: &CommissionPlace,
    key: &Segment,
) -> anyhow::Result<Option<Node>> {
    let only = Some(key);
    let nodes = match place {
        CommissionPlace::Commission(commission) => fixed_entries(*commission),
        CommissionPlace::Attachments(commission) => attachments(context, *commission, only).await?,
        CommissionPlace::Slots(commission) => slots(context, *commission, only).await?,
        CommissionPlace::UserCommissions(_)
        | CommissionPlace::AccountCommissions(_)
        | CommissionPlace::Changelog(_)
        | CommissionPlace::Products(_)
        | CommissionPlace::Slot(..)
        | CommissionPlace::File(..) => Vec::new(),
    };
    let found = nodes
        .into_iter()
        .find(|node| node.place.key().as_ref() == Some(key));
    Ok(found)
}

/// Every real child of the folder `place`. `products` is empty because nothing can be stored
/// in it yet; the slice that adds Products replaces that answer. An Account's `commissions`
/// is empty because bringing a commission into an Account isn't modeled yet.
pub(crate) async fn children(
    context: &DenContext<'_>,
    place: &CommissionPlace,
) -> anyhow::Result<Vec<Node>> {
    match place {
        CommissionPlace::Commission(commission) => Ok(fixed_entries(*commission)),
        CommissionPlace::Attachments(commission) => attachments(context, *commission, None).await,
        CommissionPlace::Slots(commission) => slots(context, *commission, None).await,
        CommissionPlace::Products(_)
        | CommissionPlace::UserCommissions(_)
        | CommissionPlace::AccountCommissions(_)
        | CommissionPlace::Changelog(_)
        | CommissionPlace::Slot(..)
        | CommissionPlace::File(..) => Ok(Vec::new()),
    }
}

/// The viewer's mounts in the folder `place`: in their own `commissions` folder, one per
/// commission they take part in, archived ones included, keyed by the commission's id.
pub(crate) async fn mounts(
    context: &DenContext<'_>,
    place: &CommissionPlace,
) -> anyhow::Result<Vec<DenMount>> {
    match place {
        CommissionPlace::UserCommissions(user) if user == context.viewer => {
            let taking_part = context
                .ports
                .commissions
                .list_participating(context.viewer)
                .await?;
            let mounts = taking_part
                .into_iter()
                .filter_map(|summary| {
                    let segment = names::id_segment(summary.id)?;
                    let target = Place::Commission(CommissionPlace::Commission(summary.id));
                    Some(DenMount { segment, target })
                })
                .collect();
            Ok(mounts)
        }
        CommissionPlace::UserCommissions(_)
        | CommissionPlace::AccountCommissions(_)
        | CommissionPlace::Commission(_)
        | CommissionPlace::Attachments(_)
        | CommissionPlace::Changelog(_)
        | CommissionPlace::Products(_)
        | CommissionPlace::Slots(_)
        | CommissionPlace::Slot(..)
        | CommissionPlace::File(..) => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests;
