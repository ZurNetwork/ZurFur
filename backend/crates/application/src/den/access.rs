//! Den's access check for the reads that bypass the Den: may this viewer open the node a
//! read serves? Answered by Atta's own resolve, as "opens" or the one not-found.

use atta::{Caps, RealPath, Segment, View};
use domain::elements::{
    account::AccountId,
    commission::{CommissionId, FileKey},
    user::UserId,
};

use crate::{
    Ports,
    account::den::AccountPlace,
    commission::den::CommissionPlace,
    den::{DenError, Place, host::DenHost, read::Reader},
    user::den::UserPlace,
};

/// A node a read that bypasses the Den serves, named by its typed ids; its real path never
/// leaves the server. Its kind's name is what the check's debug event logs.
#[derive(Debug, Clone, PartialEq, Eq, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub(crate) enum ServedNode {
    /// The session viewer's own root.
    OwnRoot,
    /// An Account's directory.
    Account(AccountId),
    /// A commission's directory.
    Commission(CommissionId),
    /// A commission's `changelog` file.
    Changelog(CommissionId),
    /// A file in a commission's `attachments`.
    File(CommissionId, FileKey),
}

/// Proof that the viewer opens the node it carries; only [`may_open`] makes one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Opens(ServedNode);

impl Opens {
    /// The node the check opened; a read serves this node, by these ids.
    pub(crate) fn node(&self) -> &ServedNode {
        &self.0
    }
}

impl ServedNode {
    /// The node's anchor, directly under a top-level directory, and the places of the steps
    /// below it down to the node.
    fn places(&self, viewer: &UserId) -> (Place, Vec<Place>) {
        match self {
            ServedNode::OwnRoot => {
                let root = Place::User(UserPlace::User(viewer.clone()));
                (root, Vec::new())
            }
            ServedNode::Account(account) => {
                let account = Place::Account(AccountPlace::Account(account.clone()));
                (account, Vec::new())
            }
            ServedNode::Commission(commission) => (commission_anchor(*commission), Vec::new()),
            ServedNode::Changelog(commission) => {
                let changelog = Place::Commission(CommissionPlace::Changelog(*commission));
                (commission_anchor(*commission), vec![changelog])
            }
            ServedNode::File(commission, file) => {
                let attachments = Place::Commission(CommissionPlace::Attachments(*commission));
                let file = Place::Commission(CommissionPlace::File(*commission, *file));
                (commission_anchor(*commission), vec![attachments, file])
            }
        }
    }
}

/// A commission's directory, `/commission/<id>`.
fn commission_anchor(commission: CommissionId) -> Place {
    Place::Commission(CommissionPlace::Commission(commission))
}

/// Why the check refused a node, by the name its debug event logs. Every refusal but a store
/// failure is the one [`DenError::NotFound`].
#[derive(Debug, strum::IntoStaticStr)]
enum Refusal {
    /// The node's anchor has no real path directly under a top-level directory.
    #[strum(serialize = "not_found.unanchored")]
    Unanchored,
    /// A step below the anchor has no key.
    #[strum(serialize = "not_found.unkeyed_step")]
    UnkeyedStep,
    /// Atta's walk found nothing the viewer may stand on.
    #[strum(serialize = "not_found.walk")]
    Walk,
    /// The walk landed on a card.
    #[strum(serialize = "not_found.card")]
    Card,
    /// The walk landed open, but somewhere other than the served node.
    #[strum(serialize = "not_found.elsewhere")]
    Elsewhere,
    /// A store failed.
    #[strum(serialize = "store_failed")]
    StoreFailed(anyhow::Error),
}

impl From<DenError> for Refusal {
    /// A walk's not-found, or its store failure.
    fn from(error: DenError) -> Self {
        match error {
            DenError::NotFound => Self::Walk,
            DenError::Store(cause) => Self::StoreFailed(cause),
        }
    }
}

impl From<Refusal> for DenError {
    /// The one not-found, or the store failure it carries.
    fn from(refusal: Refusal) -> Self {
        match refusal {
            Refusal::StoreFailed(cause) => Self::Store(cause),
            Refusal::Unanchored
            | Refusal::UnkeyedStep
            | Refusal::Walk
            | Refusal::Card
            | Refusal::Elsewhere => Self::NotFound,
        }
    }
}

/// Whether `viewer` opens `node`: Atta's own resolve from the node's anchor, landing open on
/// exactly the anchor's real path plus the steps. A card, a mount, or any miss is the one
/// [`DenError::NotFound`]; a store failure stays [`DenError::Store`]. `include_deleted` is
/// off and doesn't matter here: a path walk's admission never reads it. Logs the node's kind
/// and the answer's class at debug.
pub(crate) async fn may_open(
    ports: &Ports,
    viewer: &UserId,
    node: ServedNode,
) -> Result<Opens, DenError> {
    let answer = checked(ports, viewer, &node).await;
    let kind: &'static str = (&node).into();
    let class = answer
        .as_ref()
        .map_or_else(<&'static str>::from, |()| "opens");
    tracing::debug!(node = kind, answer = class, "den check answered");
    answer?;
    Ok(Opens(node))
}

/// The check behind [`may_open`], refusing with its reason.
async fn checked(ports: &Ports, viewer: &UserId, node: &ServedNode) -> Result<(), Refusal> {
    let (anchor, steps) = node.places(viewer);
    let anchor = anchor.anchored_path().ok_or(Refusal::Unanchored)?;
    let steps = steps
        .iter()
        .map(Place::key)
        .collect::<Option<Vec<Segment>>>()
        .ok_or(Refusal::UnkeyedStep)?;
    opens_at(ports, viewer, &anchor, &steps).await
}

/// Whether `viewer` opens the node at `steps` below `anchor`, a real path directly under a
/// top-level directory: resolved open, on exactly `anchor` plus `steps`.
async fn opens_at(
    ports: &Ports,
    viewer: &UserId,
    anchor: &RealPath,
    steps: &[Segment],
) -> Result<(), Refusal> {
    let host = DenHost::for_check(ports, viewer);
    let reader = Reader::over(&host, Caps::default());
    let resolved = reader.walk(anchor, steps).await?;
    let View::Open { real_path, .. } = &resolved.node().view else {
        return Err(Refusal::Card);
    };
    let served: RealPath = anchor.segments().iter().chain(steps).cloned().collect();
    if real_path != &served {
        return Err(Refusal::Elsewhere);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
