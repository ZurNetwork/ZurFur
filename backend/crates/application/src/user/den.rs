//! Identity's arm of the Den for a user's directory: the node at `/user/<did>` and the
//! folders inside it that identity answers for.

use domain::elements::user::UserId;

use crate::den::{
    DenContext, DenMount, DenType, Node, Segment, TopLevel,
    names::{self, ACCOUNTS, CHARACTERS},
};

/// The places in a user's directory that this arm answers for. The user's `commissions`
/// folder is the commission arm's (`CommissionPlace::UserCommissions`), and their `posts`
/// entry is a mount of `/post/<did>`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "the identity arms build these once they serve a user's nodes"
)]
pub(crate) enum UserPlace {
    /// The user's own directory, `/user/<did>`.
    User(UserId),
    /// The user's `accounts` folder.
    Accounts(UserId),
    /// The user's `characters` folder.
    Characters(UserId),
}

impl UserPlace {
    /// The type of the node at this place.
    pub(crate) fn den_type(&self) -> DenType {
        match self {
            UserPlace::User(_) => DenType::User,
            UserPlace::Accounts(_) => DenType::UserAccounts,
            UserPlace::Characters(_) => DenType::UserCharacters,
        }
    }

    /// The node's segment in its real parent.
    pub(crate) fn key(&self) -> Option<Segment> {
        match self {
            UserPlace::User(user) => names::did_segment(user.did()),
            UserPlace::Accounts(_) => names::fixed_segment(ACCOUNTS),
            UserPlace::Characters(_) => names::fixed_segment(CHARACTERS),
        }
    }

    /// The top-level directory the node sits directly under, if it does.
    pub(crate) fn top_level(&self) -> Option<TopLevel> {
        match self {
            UserPlace::User(_) => Some(TopLevel::User),
            UserPlace::Accounts(_) | UserPlace::Characters(_) => None,
        }
    }
}

/// The user at `/user/<user>`, for the session's viewer. Not served yet: always absent.
pub(crate) async fn node(
    _context: &DenContext<'_>,
    _user: &UserId,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// The node at `key` in the folder `place`. Not served yet: always absent.
pub(crate) async fn child(
    _context: &DenContext<'_>,
    _place: &UserPlace,
    _key: &Segment,
) -> anyhow::Result<Option<Node>> {
    Ok(None)
}

/// Every real child of the folder `place`. Not served yet: always empty.
pub(crate) async fn children(
    _context: &DenContext<'_>,
    _place: &UserPlace,
) -> anyhow::Result<Vec<Node>> {
    Ok(Vec::new())
}

/// The viewer's mounts in the folder `place`. Not served yet: always empty.
pub(crate) async fn mounts(
    _context: &DenContext<'_>,
    _place: &UserPlace,
) -> anyhow::Result<Vec<DenMount>> {
    Ok(Vec::new())
}
