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
/// leaves the server.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "identity's and commission's reads name their nodes once they move onto the check"
)]
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

/// Proof that the viewer opens the node they asked about; only [`may_open`] makes one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Opens(());

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

/// Whether `viewer` opens `node`: Atta's own resolve from the node's anchor, landing open on
/// exactly the anchor's real path plus the steps. A card, a mount, or any miss is the one
/// [`DenError::NotFound`]; a store failure stays [`DenError::Store`].
#[allow(
    dead_code,
    reason = "identity's and commission's reads call it once they move onto the check"
)]
pub(crate) async fn may_open(
    ports: &Ports,
    viewer: &UserId,
    node: &ServedNode,
) -> Result<Opens, DenError> {
    let (anchor, steps) = node.places(viewer);
    let anchor = anchor.anchored_path().ok_or(DenError::NotFound)?;
    let steps = steps
        .iter()
        .map(Place::key)
        .collect::<Option<Vec<Segment>>>()
        .ok_or(DenError::NotFound)?;
    let host = DenHost::new(ports, viewer, false);
    let reader = Reader::over(&host, Caps::default());
    let resolved = reader.walk(&anchor, &steps).await?;
    let View::Open { real_path, .. } = &resolved.node().view else {
        return Err(DenError::NotFound);
    };
    let served: RealPath = anchor.segments().iter().chain(&steps).cloned().collect();
    if real_path != &served {
        return Err(DenError::NotFound);
    }
    Ok(Opens(()))
}

#[cfg(test)]
mod tests;
