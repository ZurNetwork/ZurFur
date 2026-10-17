use crate::elements::{
    did::Did,
    role::{Role, RoleAlias},
};

use super::Account;

/// One row of [`crate::ports::AccountStore::list_for_user`]: a live [`Account`]
/// paired with the caller's own [`Role`] on it. Distinct from [`UserAccount`],
/// which addresses a membership for writes; this carries the full account row a
/// listing renders.
pub struct AccountMembership {
    /// The live account itself — never a soft-deleted one.
    pub account: Account,
    /// The caller's own standing on that account, not the account's owner.
    pub role: Role,
    /// The caller's own [`RoleAlias`] for that role, if they set one.
    pub alias: Option<RoleAlias>,
}

/// Who a membership listing is for — and therefore whether the
/// `listed_on_profile` privacy valve applies. Required (no default, no
/// `Option`) on
/// [`list_for_user`](crate::ports::AccountStore::list_for_user), so the valve
/// cannot be bypassed by omission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingScope {
    /// The user reading their own accounts — every live membership,
    /// `listed_on_profile` ignored.
    SelfView,
    /// A public projection of someone's memberships — honors
    /// `listed_on_profile`, so an unlisted membership is absent.
    PublicProfile,
}

/// An account's public-facing profile: its [`Did`] and a display name — the
/// public projection of an [`Account`], distinct from the private row.
pub struct AccountProfile {
    pub did: Did,
    pub display_name: String,
}
