//! The [`UserAccount`] — a membership: which [`Role`] a user holds in an account.
//!
//! On this platform a membership *is* the join: granting a role is how a user
//! joins an account, revoking it is how they leave (DESIGN/Roles). This is the
//! row persisted alongside a new account's founder (ZMVP-14) and the unit
//! [`crate::ports::AccountRepo::grant_role`] upserts.

use crate::elements::{account::AccountId, role::Role, user::UserId};

/// A user's membership in an account: the `(user_id, account_id, role)` triple.
///
/// Plain public named fields — `user_id`, `account_id`, and `role`. One user may
/// be a member of many accounts, so a [`UserId`] is unique only together with its
/// [`AccountId`].
///
/// References: [`Role`], [`crate::elements::account::Account::open`] (which mints
/// the founder's `UserAccount`), [`crate::ports::AccountRepo`].
pub struct UserAccount {
    pub user_id: UserId,
    pub account_id: AccountId,
    pub role: Role,
}
