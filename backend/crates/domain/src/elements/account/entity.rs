use crate::{
    datetime::DateTimeUtc,
    elements::{did::Did, handle::Handle, role::Role, user::UserId, user_account::UserAccount},
};

use super::{AccountId, AccountName};
pub use crate::elements::did::DeleteOutcome;

/// A founded account: its [`AccountId`], a [`Handle`], a validated
/// [`AccountName`], and lifecycle timestamps. Build one with [`Account::open`],
/// which also mints the founder's Owner membership. `deleted_at` is the
/// soft-delete marker — a deleted account keeps its row but
/// [`crate::ports::AccountStore::find`] returns `None` for it.
pub struct Account {
    pub id: AccountId,
    /// The public handle the account is reached by, chosen at founding and
    /// unique across all accounts — a soft-deleted one still reserves its
    /// handle.
    pub handle: Handle,
    /// The name the founder gave the account.
    pub name: AccountName,
    /// When the account was founded; equals `updated_at` at creation.
    pub created_at: DateTimeUtc,
    /// When the account was last changed.
    pub updated_at: DateTimeUtc,
    /// Soft-delete marker: `Some(when)` once deleted, else `None`.
    pub deleted_at: Option<DateTimeUtc>,
}

impl Account {
    /// Open an account and seat its founder as Owner — the two are never
    /// created apart. Stamps `created_at == updated_at == now`; `name`, `handle`
    /// and `did` all arrive already validated or minted.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{account::{Account, AccountName}, did::Did, handle::Handle, role::Role, user::UserId};
    ///
    /// let owner = UserId::from(Did::from("did:plc:owner".to_string()));
    /// let (account, membership) = Account::open(
    ///     owner,
    ///     Did::from("did:plc:example".to_string()),
    ///     "acme.zurfur.app".parse::<Handle>().unwrap(),
    ///     "Acme Studio".parse::<AccountName>().unwrap(),
    ///     Utc::now(),
    /// );
    /// assert_eq!(membership.role, Role::Owner); // founder is Owner
    /// assert_eq!(account.handle.as_str(), "acme.zurfur.app"); // reached by its handle
    /// assert_eq!(account.created_at, account.updated_at);   // stamped once
    /// ```
    pub fn open(
        owner: UserId,
        did: Did,
        handle: Handle,
        name: AccountName,
        now: DateTimeUtc,
    ) -> (Account, UserAccount) {
        let new_account = Account {
            id: AccountId::from(did),
            handle,
            name,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        let membership = UserAccount {
            user_id: owner,
            account_id: new_account.id.clone(),
            role: Role::Owner,
            alias: None,
        };
        (new_account, membership)
    }
}
