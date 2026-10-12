//! Identity's arm of the Den for an Account's directory: the node at `/account/<did>` and
//! the folders inside it that identity answers for.

use domain::elements::account::AccountId;

use crate::den::{
    DenContext, DenMount, DenType, Node, Segment, TopLevel,
    names::{self, CHARACTERS},
};

/// The places in an Account's directory that this arm answers for. The Account's
/// `commissions` folder is the commission arm's (`CommissionPlace::AccountCommissions`), and
/// its `workflows` folder is the Workflow arm's (`WorkflowPlace::Workflows`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[expect(
    dead_code,
    reason = "the identity arms build these once they serve an Account's nodes"
)]
pub(crate) enum AccountPlace {
    /// The Account's directory, `/account/<did>`.
    Account(AccountId),
    /// The Account's `characters` folder.
    Characters(AccountId),
}

impl AccountPlace {
    /// The type of the node at this place.
    pub(crate) fn den_type(&self) -> DenType {
        match self {
            AccountPlace::Account(_) => DenType::Account,
            AccountPlace::Characters(_) => DenType::AccountCharacters,
        }
    }

    /// The node's segment in its real parent.
    pub(crate) fn key(&self) -> Option<Segment> {
        match self {
            AccountPlace::Account(account) => Segment::try_from(account.as_ref()).ok(),
            AccountPlace::Characters(_) => names::fixed_segment(CHARACTERS),
        }
    }

    /// The top-level directory the node sits directly under, if it does.
    pub(crate) fn top_level(&self) -> Option<TopLevel> {
        match self {
            AccountPlace::Account(_) => Some(TopLevel::Account),
            AccountPlace::Characters(_) => None,
        }
    }
}

/// The Account at `/account/<account>`, for the session's viewer. Not served yet: always
/// absent.
pub(crate) async fn node(
    _context: &DenContext<'_>,
    _account: &AccountId,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// The node at `key` in the folder `place`. Not served yet: always absent.
pub(crate) async fn child(
    _context: &DenContext<'_>,
    _place: &AccountPlace,
    _key: &Segment,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// Every real child of the folder `place`. Not served yet: always empty.
pub(crate) async fn children(
    _context: &DenContext<'_>,
    _place: &AccountPlace,
) -> anyhow::Result<Vec<Node>> {
    Ok(Vec::new())
}

/// The viewer's mounts in the folder `place`. Not served yet: always empty.
pub(crate) async fn mounts(
    _context: &DenContext<'_>,
    _place: &AccountPlace,
) -> anyhow::Result<Vec<DenMount>> {
    Ok(Vec::new())
}
