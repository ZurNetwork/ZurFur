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
#[allow(
    dead_code,
    reason = "the identity and commission arms lift their viewers with it"
)]
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
    reason = "every variant but `TopLevel` is built by its team's arms once they serve nodes"
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
#[allow(
    dead_code,
    reason = "built by the identity and commission arms once they mount nodes"
)]
pub(crate) struct DenMount {
    /// Its segment in the folder.
    pub(crate) segment: Segment,
    /// The node it grafts in; a place not directly under a top-level directory is dropped.
    pub(crate) target: Place,
}
