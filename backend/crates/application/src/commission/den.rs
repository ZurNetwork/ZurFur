//! Commission's arm of the Den: the node at `/commission/<id>`, the entries inside it, and
//! the folders of commission mounts in a user's and an Account's directory.

use domain::elements::{
    account::AccountId,
    commission::{CommissionId, ElementId, FileKey},
    user::UserId,
};

use crate::den::{
    DenContext, DenMount, DenType, Node, Segment, TopLevel,
    names::{self, ATTACHMENTS, CHANGELOG, COMMISSIONS, PRODUCTS, SLOTS},
};

/// The places this arm answers for.
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    dead_code,
    reason = "the commission arms build these once they serve commission nodes"
)]
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
            CommissionPlace::Commission(commission) => {
                Segment::try_from(commission.to_string()).ok()
            }
            CommissionPlace::Attachments(_) => names::fixed_segment(ATTACHMENTS),
            CommissionPlace::Changelog(_) => names::fixed_segment(CHANGELOG),
            CommissionPlace::Products(_) => names::fixed_segment(PRODUCTS),
            CommissionPlace::Slots(_) => names::fixed_segment(SLOTS),
            CommissionPlace::Slot(_, slot) => Segment::try_from(slot.to_string()).ok(),
            CommissionPlace::File(_, file) => Segment::try_from(file.to_string()).ok(),
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

/// The commission at `/commission/<commission>`, for the session's viewer. Not served yet:
/// always absent.
pub(crate) async fn node(
    _context: &DenContext<'_>,
    _commission: CommissionId,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// The node at `key` in the folder `place`. Not served yet: always absent.
pub(crate) async fn child(
    _context: &DenContext<'_>,
    _place: &CommissionPlace,
    _key: &Segment,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// Every real child of the folder `place`. Not served yet: always empty.
pub(crate) async fn children(
    _context: &DenContext<'_>,
    _place: &CommissionPlace,
) -> anyhow::Result<Vec<Node>> {
    Ok(Vec::new())
}

/// The viewer's mounts in the folder `place`. Not served yet: always empty.
pub(crate) async fn mounts(
    _context: &DenContext<'_>,
    _place: &CommissionPlace,
) -> anyhow::Result<Vec<DenMount>> {
    Ok(Vec::new())
}
