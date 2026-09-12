//! The [`UserAccount`] — a membership: which [`Role`] a user holds in an account.
//!
//! On this platform a membership *is* the join: granting a role is how a user
//! joins an account, revoking it is how they leave (DESIGN/Roles). This is the
//! row persisted alongside a new account's founder (ZMVP-14) and the unit
//! [`crate::ports::AccountWrites::grant_role`] upserts.

use crate::elements::{
    account::AccountId,
    role::{Role, RoleAlias},
    user::UserId,
};

/// A user's membership in an account: the `(user_id, account_id, role)` triple,
/// plus the member's own optional [`RoleAlias`] for that role.
///
/// Plain public named fields — `user_id`, `account_id`, `role`, and `alias`. One
/// user may be a member of many accounts, so a [`UserId`] is unique only together
/// with its [`AccountId`]. The alias is a free-form label the member chose for
/// their own role (e.g. an Owner aliased "Studio Head") — it carries no
/// authority and never influences [`Role::can_grant`]; `None` on the floor.
///
/// References: [`Role`], [`RoleAlias`], [`crate::elements::account::Account::open`]
/// (which mints the founder's `UserAccount`), [`crate::ports::AccountWrites`].
pub struct UserAccount {
    pub user_id: UserId,
    pub account_id: AccountId,
    pub role: Role,
    pub alias: Option<RoleAlias>,
}
