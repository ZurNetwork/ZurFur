//! The [`Account`] — a platform-custodied entity that is its own sovereign
//! identity. (DESIGN 1966081)
//!
//! An account holds a minted `did:plc` of its own, a validated human name, a
//! handle, and soft-delete timestamps. It is founded together with its founder's
//! Owner membership in a single act, [`Account::open`].

use std::{ops::Deref, str::FromStr};

use serde::Deserialize;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        did::Did,
        handle::Handle,
        id::IdError,
        role::{Role, RoleAlias},
        user::UserId,
        user_account::UserAccount,
    },
    string_builder::{StringBuilder, StringBuilderViolation},
};

/// An [`Account`]'s identifier: its [`Did`]. (DD 57081857)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
#[serde(transparent)]
pub struct AccountId(Did);

impl AccountId {
    /// Wraps an already-minted DID.
    pub fn new(id: Did) -> Self {
        Self(id)
    }
}

impl Deref for AccountId {
    type Target = Did;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for AccountId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let id = s
            .parse::<Did>()
            .map(Self)
            .map_err(|_| IdError::ParsingError)?;
        Ok(id)
    }
}

/// The longest an account name may be, in `char`s (counted after trimming).
pub const ACCOUNT_NAME_MAX_LEN: usize = 120;

/// A human-readable account name: trimmed, non-empty, at most
/// [`ACCOUNT_NAME_MAX_LEN`] chars.
///
/// ```
/// use domain::elements::account::AccountName;
///
/// let name = "  Acme Studio  ".parse::<AccountName>().unwrap();
/// assert_eq!(name.as_str(), "Acme Studio"); // trimmed
///
/// assert!("   ".parse::<AccountName>().is_err()); // empty after trim
/// assert!("x".repeat(121).parse::<AccountName>().is_err()); // too long
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountName(String);

/// Why a string was rejected as an account name.
///
/// ```
/// use domain::elements::account::{AccountName, AccountNameError};
///
/// assert_eq!("".parse::<AccountName>(), Err(AccountNameError::Empty));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountNameError {
    /// Empty once trimmed.
    Empty,
    /// Longer than [`ACCOUNT_NAME_MAX_LEN`] chars; carries the length.
    TooLong(usize),
}

impl std::fmt::Display for AccountNameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountNameError::Empty => write!(f, "account name must not be empty"),
            AccountNameError::TooLong(len) => write!(
                f,
                "account name is {len} chars; the max is {ACCOUNT_NAME_MAX_LEN}"
            ),
        }
    }
}

impl std::error::Error for AccountNameError {}

impl AccountName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The one validating constructor: trim, then check the bounds above.
impl std::str::FromStr for AccountName {
    type Err = AccountNameError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        StringBuilder::new(raw)
            .trimmed()
            .non_empty()
            .max_chars(ACCOUNT_NAME_MAX_LEN)
            .build()
            .map(Self)
            .map_err(|violation| match violation {
                StringBuilderViolation::Empty => AccountNameError::Empty,
                StringBuilderViolation::TooLong { len, .. } => AccountNameError::TooLong(len),
                StringBuilderViolation::ControlCharacter => {
                    // Unreachable: this chain never calls no_control.
                    debug_assert!(
                        false,
                        "AccountName's FromStr chain never calls no_control; ControlCharacter is unreachable"
                    );
                    AccountNameError::Empty
                }
            })
    }
}

impl AsRef<str> for AccountName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for AccountName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A founded account: its [`AccountId`], a [`Handle`], a validated
/// [`AccountName`], and lifecycle timestamps. Build one with [`Account::open`],
/// which also mints the founder's Owner membership. `deleted_at` is the
/// soft-delete marker — a deleted account keeps its row but
/// [`crate::ports::AccountStore::find`] returns `None` for it.
pub struct Account {
    pub id: AccountId,
    /// The public handle the account is reached by, chosen at founding and
    /// unique across all accounts — a soft-deleted one still reserves its
    /// handle. (DD 23003138)
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
    /// let owner = UserId::new(Did::new("did:plc:owner".to_string()));
    /// let (account, membership) = Account::open(
    ///     owner,
    ///     Did::new("did:plc:example".to_string()),
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
            id: AccountId::new(did),
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
/// cannot be bypassed by omission. (DD 21594113)
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
