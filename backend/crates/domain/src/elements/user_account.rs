//! The [`UserAccount`] — a membership: which [`Role`] a user holds in an
//! account. A membership IS the join: granting a role is how a user joins,
//! revoking it is how they leave. (DESIGN 2162692)

use crate::elements::{
    account::AccountId,
    role::{Role, RoleAlias},
    user::UserId,
};

/// A user's membership in an account: the `(user_id, account_id, role)` triple
/// plus the member's own optional [`RoleAlias`]. One user may be a member of
/// many accounts, so a [`UserId`] is unique only with its [`AccountId`]. The
/// alias carries no authority and never influences [`Role::can_grant`].
pub struct UserAccount {
    pub user_id: UserId,
    pub account_id: AccountId,
    pub role: Role,
    pub alias: Option<RoleAlias>,
}
