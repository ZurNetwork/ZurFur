use atta::{Level, Lift, NodeName, RealPath, Relationship, Scope, Segment};

use crate::{
    account::{den::AccountPlace, workflow::den::WorkflowPlace},
    character::den::CharacterPlace,
    commission::den::CommissionPlace,
    den::{DenType, NodeState, names, post::PostPlace},
    user::den::UserPlace,
};

/// Standing over a node's whole real subtree: the lift a user has at their own root, a member
/// at their Account, a keeper at their Character and a Participant at their commission.
pub(crate) const STANDING: Lift = Lift {
    level: Level::Public,
    scope: Scope::Subtree,
    relationship: Relationship::Standing,
};

/// The top-level directories under `/`: Public, owned by the system and traverse-only.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    strum::Display,
    strum::EnumString,
    strum::IntoStaticStr,
    strum::VariantArray,
)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum TopLevel {
    /// `/user`, keyed by a user's DID.
    User,
    /// `/account`, keyed by an Account's DID.
    Account,
    /// `/character`, keyed by a Character's DID.
    Character,
    /// `/commission`, keyed by a commission's id.
    Commission,
    /// `/post`, keyed by a poster's DID.
    Post,
}

/// Where a node sits, typed: its type and the ids the arm that answers for it needs. Each
/// variant is one team's places, defined beside that team's arm.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "the identity and posts variants are built by their arms once they serve nodes"
)]
pub(crate) enum Place {
    /// A top-level directory, den's own.
    TopLevel(TopLevel),
    /// A user's directory and the folders identity answers inside it.
    User(UserPlace),
    /// An Account's directory and the folders identity answers inside it.
    Account(AccountPlace),
    /// A Character's directory.
    Character(CharacterPlace),
    /// A commission, its entries, and the folders of commission mounts.
    Commission(CommissionPlace),
    /// An Account's `workflows` folder and its Workflows.
    Workflow(WorkflowPlace),
    /// A poster's posts.
    Post(PostPlace),
}

impl Place {
    /// The type of the node at this place.
    pub(crate) fn den_type(&self) -> DenType {
        match self {
            Place::TopLevel(_) => DenType::TopLevel,
            Place::User(place) => place.den_type(),
            Place::Account(place) => place.den_type(),
            Place::Character(place) => place.den_type(),
            Place::Commission(place) => place.den_type(),
            Place::Workflow(place) => place.den_type(),
            Place::Post(place) => place.den_type(),
        }
    }

    /// The node's key, its segment in its real parent; `None` when the id it is built from
    /// isn't a valid segment, which makes the node absent.
    pub(crate) fn key(&self) -> Option<Segment> {
        match self {
            Place::TopLevel(top_level) => names::fixed_segment(top_level.into()),
            Place::User(place) => place.key(),
            Place::Account(place) => place.key(),
            Place::Character(place) => place.key(),
            Place::Commission(place) => place.key(),
            Place::Workflow(place) => place.key(),
            Place::Post(place) => place.key(),
        }
    }

    /// The top-level directory the node sits directly under, if it does: only such a node
    /// can be a mount's target.
    pub(crate) fn top_level(&self) -> Option<TopLevel> {
        match self {
            Place::TopLevel(_) => None,
            Place::User(place) => place.top_level(),
            Place::Account(place) => place.top_level(),
            Place::Character(place) => place.top_level(),
            Place::Commission(place) => place.top_level(),
            Place::Workflow(place) => place.top_level(),
            Place::Post(place) => place.top_level(),
        }
    }

    /// The real path of a node directly under a top-level directory; `None` for any other.
    pub(crate) fn anchored_path(&self) -> Option<RealPath> {
        let top_level = names::fixed_segment(self.top_level()?.into())?;
        let key = self.key()?;
        let path = RealPath::from(vec![top_level, key]);
        Some(path)
    }
}

/// Where a folder sits: `/` itself, or a node the host answered for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Parent {
    /// The global root, `/`.
    Root,
    /// A node at this place.
    At(Place),
}

impl Place {
    /// Whether a node at this place may be a real child of `parent`: `/` holds only the
    /// top-level directories, a top-level directory only the nodes anchored under it, and each
    /// directory only its own fixed entries or its own entities.
    pub(crate) fn is_child_of(&self, parent: &Parent) -> bool {
        let Parent::At(parent) = parent else {
            return matches!(self, Place::TopLevel(_));
        };
        match parent {
            Place::TopLevel(top_level) => self.top_level() == Some(*top_level),
            Place::User(UserPlace::User(user)) => match self {
                Place::User(UserPlace::Accounts(child) | UserPlace::Characters(child)) => {
                    child == user
                }
                Place::Commission(CommissionPlace::UserCommissions(child)) => child == user,
                Place::TopLevel(_)
                | Place::User(UserPlace::User(_))
                | Place::Account(_)
                | Place::Character(_)
                | Place::Commission(_)
                | Place::Workflow(_)
                | Place::Post(_) => false,
            },
            Place::Account(AccountPlace::Account(account)) => match self {
                Place::Account(AccountPlace::Characters(child))
                | Place::Commission(CommissionPlace::AccountCommissions(child))
                | Place::Workflow(WorkflowPlace::Workflows(child)) => child == account,
                Place::TopLevel(_)
                | Place::User(_)
                | Place::Account(AccountPlace::Account(_))
                | Place::Character(_)
                | Place::Commission(_)
                | Place::Workflow(_)
                | Place::Post(_) => false,
            },
            Place::Commission(CommissionPlace::Commission(commission)) => match self {
                Place::Commission(
                    CommissionPlace::Attachments(child)
                    | CommissionPlace::Changelog(child)
                    | CommissionPlace::Products(child)
                    | CommissionPlace::Slots(child),
                ) => child == commission,
                Place::TopLevel(_)
                | Place::User(_)
                | Place::Account(_)
                | Place::Character(_)
                | Place::Commission(_)
                | Place::Workflow(_)
                | Place::Post(_) => false,
            },
            Place::Commission(CommissionPlace::Attachments(commission)) => {
                matches!(self, Place::Commission(CommissionPlace::File(child, _)) if child == commission)
            }
            Place::Commission(CommissionPlace::Slots(commission)) => {
                matches!(self, Place::Commission(CommissionPlace::Slot(child, _)) if child == commission)
            }
            Place::Workflow(WorkflowPlace::Workflows(account)) => {
                matches!(self, Place::Workflow(WorkflowPlace::Workflow(child, _)) if child == account)
            }
            Place::User(UserPlace::Accounts(_) | UserPlace::Characters(_))
            | Place::Account(AccountPlace::Characters(_))
            | Place::Character(_)
            | Place::Commission(
                CommissionPlace::UserCommissions(_)
                | CommissionPlace::AccountCommissions(_)
                | CommissionPlace::Changelog(_)
                | CommissionPlace::Products(_)
                | CommissionPlace::Slot(..)
                | CommissionPlace::File(..),
            )
            | Place::Workflow(WorkflowPlace::Workflow(..))
            | Place::Post(_) => false,
        }
    }
}

/// A node as an arm answers for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Node {
    /// Where it sits; its type and key follow from it.
    pub(crate) place: Place,
    /// Its display name, from what it holds.
    pub(crate) name: NodeName,
    /// Its own level; Private when nothing is stored.
    pub(crate) level: Level,
    /// The viewer's lifts at it.
    pub(crate) lifts: Vec<Lift>,
    /// Whether it is live or soft-deleted.
    pub(crate) state: NodeState,
}

/// One of the viewer's mounts in a folder, as an arm answers for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DenMount {
    /// Its segment in the folder.
    pub(crate) segment: Segment,
    /// The node it grafts in; a place not directly under a top-level directory is dropped.
    pub(crate) target: Place,
}

#[cfg(test)]
mod tests;
