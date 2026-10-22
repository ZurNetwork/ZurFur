//! Commission's arm of the Den: the node at `/commission/<id>`, the entries inside it, and
//! the folders of commission mounts in a user's and an Account's directory.

use domain::elements::{
    account::AccountId,
    commission::{
        CommissionId, CommissionRole, CommissionStanding, CommissionSummary, ElementId, FileKey,
    },
    level::Level,
    user::UserId,
};

use crate::Ports;
use crate::den::{
    DenContext, DenMount, DenType, Node, NodeState, Place, STANDING, Segment, TopLevel,
    names::{self, ATTACHMENTS, CHANGELOG, COMMISSIONS, PRODUCTS, SLOTS},
};

/// The places this arm answers for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommissionPlace {
    /// A user's `commissions` folder, which holds a mount of each commission they are part of.
    UserCommissions(UserId),
    /// An Account's `commissions` folder, which holds a mount of each commission brought into it.
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

/// The node for a commission's summary: its node's level, standing for a Participant,
/// archived or live, and its node's name through the name rule.
fn commission_node(commission: &CommissionSummary, participant: bool) -> Option<Node> {
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
        level: commission.visibility,
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
    let mut children = Vec::new();
    for file in files {
        let place = CommissionPlace::File(commission, file.id);
        if only.is_some_and(|key| names::id_segment(file.id).as_ref() != Some(key)) {
            continue;
        }
        let stored = context.ports.files.filename(file.id).await?;
        let name = stored.map_or(names::Name::Fixed(names::UNNAMED), names::Name::Stored);
        children.extend(entry(place, name));
    }
    Ok(children)
}

/// The commission's slots, from its own slot list, each named by its title.
async fn slots(
    context: &DenContext<'_>,
    commission: CommissionId,
    only: Option<&Segment>,
) -> anyhow::Result<Vec<Node>> {
    let slots = context.ports.commissions.slots(&commission).await?;
    let children = slots
        .into_iter()
        .filter(|slot| {
            only.is_none_or(|key| names::id_segment(slot.element_id).as_ref() == Some(key))
        })
        .filter_map(|slot| {
            let place = CommissionPlace::Slot(commission, slot.element_id);
            entry(place, names::Name::Stored(slot.title))
        })
        .collect();
    Ok(children)
}

/// The commission at `/commission/<commission>`, for the session's viewer. Always reads the
/// commission's summary and then the viewer's participation, both, so an absent commission
/// and one that isn't the viewer's run the same statements.
pub(crate) async fn node(
    context: &DenContext<'_>,
    commission: CommissionId,
) -> anyhow::Result<Option<Node>> {
    let store = &context.ports.commissions;
    let found = store.find_summary(&commission).await?;
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
    let candidates = match place {
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
    let found = candidates
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

/// What a write's standing on a commission answers: the one not-found, a role below the act's
/// need, or leave to act with the facts the write needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// No such commission, or the actor holds no standing on it, whatever its level.
    NotFound,
    /// The actor's role is below what the act needs.
    Forbidden,
    /// The actor may act.
    Granted(StandingFacts),
}

/// The facts a granted standing hands the write: the actor's role, the commission's own level
/// and whether it is archived. No id: the write already names its commission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StandingFacts {
    /// The actor's role, at or above what the act needs.
    pub(crate) role: CommissionRole,
    /// The commission's own level.
    pub(crate) level: Level,
    /// Whether it is archived; archived refuses nothing yet.
    pub(crate) archived: bool,
}

/// The verdict on one standing for an act that needs `needs`: no commission or no role is the
/// one not-found, a lower role is forbidden, and at or above is granted. The pool's read and the
/// locked read both answer through it.
pub(crate) fn verdict(standing: Option<CommissionStanding>, needs: CommissionRole) -> Verdict {
    let Some(standing) = standing else {
        return Verdict::NotFound;
    };
    let Some(role) = standing.role else {
        return Verdict::NotFound;
    };
    if role < needs {
        return Verdict::Forbidden;
    }
    let facts = StandingFacts {
        role,
        level: standing.level,
        archived: standing.archived,
    };
    Verdict::Granted(facts)
}

/// The verdict on `actor`'s standing on `commission` from the pool, for an act that needs
/// `needs`: one statement, the same for a stranger and an id nothing is stored under.
pub(crate) async fn pool_verdict(
    ports: &Ports,
    commission: &CommissionId,
    actor: &UserId,
    needs: CommissionRole,
) -> anyhow::Result<Verdict> {
    let standing = ports.commissions.standing(commission, actor).await?;
    Ok(verdict(standing, needs))
}

#[cfg(test)]
mod tests;
