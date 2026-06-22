//! The [`UserAccount`] — a membership: which [`Role`] a user holds in an account.
//!
//! On this platform a membership *is* the join: granting a role is how a user
//! joins an account, revoking it is how they leave (DESIGN/Roles). This is the
//! row persisted alongside a new account's founder (ZMVP-14) and the unit
//! [`crate::ports::AccountRepo::grant_role`] upserts.

use crate::elements::{account::AccountId, role::Role, user::UserId};

/// A user's membership in an account, as the `(user, account, role)` tuple.
///
/// A positional tuple struct rather than named fields — read it through the
/// accessors below ([`get_user_id`](UserAccount::get_user_id),
/// [`get_account_id`](UserAccount::get_account_id),
/// [`get_role`](UserAccount::get_role)). One user may be a member of many
/// accounts, so a [`UserId`] is unique only together with its [`AccountId`].
///
/// References: [`Role`], [`crate::elements::account::Account::open`] (which mints
/// the founder's `UserAccount`), [`crate::ports::AccountRepo`].
pub struct UserAccount(pub UserId, pub AccountId, pub Role);

impl UserAccount {
    /// The member.
    pub fn get_user_id(&self) -> UserId {
        self.0
    }

    /// The account they are a member of.
    pub fn get_account_id(&self) -> AccountId {
        self.1
    }

    /// The role they hold (cloned, since [`Role`] carries an owned parent slot).
    pub fn get_role(&self) -> Role {
        self.2.clone()
    }
}
