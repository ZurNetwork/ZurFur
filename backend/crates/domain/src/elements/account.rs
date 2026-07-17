//! The [`Account`] — a platform-custodied entity that is its own sovereign
//! identity (DESIGN/Account).
//!
//! An account holds a minted `did:plc` of its own (unlike a visitor's DID, which
//! precedes us), a validated human name, and soft-delete timestamps. It is
//! founded together with its founder's Owner membership in a single act,
//! [`Account::open`] — the ZMVP-14 invariant "the creating User becomes Owner."
//! Persisting the pair is one private-side transaction
//! ([`crate::ports::AccountWrites::create`]).

use std::ops::Deref;

use crate::{
    datetime::DateTimeUtc,
    elements::{did::Did, handle::Handle, role::Role, user::UserId, user_account::UserAccount},
    string_builder::{StringBuilder, StringBuilderViolation},
};

/// The app-private, stable handle for an [`Account`].
///
/// A UUIDv7 wrapped for type safety, mirroring [`crate::elements::user::UserId`].
/// The account's *public* identity is its [`Did`]; this id is the private key
/// used for foreign keys and lookups. Deref exposes the inner UUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AccountId(uuid::Uuid);

impl AccountId {
    /// Wraps an already-minted UUIDv7. Mirrors [`crate::elements::user::UserId::new`]:
    /// the app mints the key (PG16 has no native `uuidv7()`), the domain only names it.
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl Deref for AccountId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The longest an account name may be, in `char`s (counted after trimming).
pub const ACCOUNT_NAME_MAX_LEN: usize = 120;

/// A human-readable account name, validated on the way in.
///
/// Surrounding whitespace is trimmed. The result must be non-empty and at most
/// [`ACCOUNT_NAME_MAX_LEN`] chars — this is the anti-spam gate: opening an account
/// demands real input, not a bare click.
///
/// ```
/// use domain::elements::account::AccountName;
///
/// let name = "  Acme Studio  ".parse::<AccountName>().unwrap();
/// assert_eq!(name.as_str(), "Acme Studio"); // trimmed
///
/// assert!("   ".parse::<AccountName>().is_err()); // empty after trim
/// assert!(AccountName::try_from("x".repeat(121)).is_err()); // too long
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
    /// Empty once trimmed. Example: `""` or `"   "`.
    Empty,
    /// Longer than [`ACCOUNT_NAME_MAX_LEN`] chars. Carries the offending length.
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

impl TryFrom<String> for AccountName {
    type Error = AccountNameError;

    /// Validate and wrap a name: trim first, then check the bounds above.
    fn try_from(raw: String) -> Result<Self, Self::Error> {
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
                    // Unreachable by construction: this chain never calls
                    // `no_control`/`no_control_except`. Fail safe onto the
                    // most conservative existing variant rather than panic.
                    debug_assert!(
                        false,
                        "AccountName's TryFrom chain never calls no_control; ControlCharacter is unreachable"
                    );
                    AccountNameError::Empty
                }
            })
    }
}

/// The std parsing door: `"…".parse::<AccountName>()?` — delegates to the
/// [`TryFrom<String>`] rules (ruling R6: `FromStr` for string parsing).
impl std::str::FromStr for AccountName {
    type Err = AccountNameError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::try_from(raw.to_owned())
    }
}

/// The std read-side view: any `impl AsRef<str>` bound accepts the newtype
/// directly (ruling R6); [`as_str`](Self::as_str) stays the explicit accessor.
impl AsRef<str> for AccountName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// A founded account: its sovereign [`Did`], its app-private [`AccountId`], a
/// validated [`AccountName`], and lifecycle timestamps.
///
/// Build one with [`Account::open`], which also mints the founder's Owner
/// membership — the two are never created apart. `deleted_at` is the soft-delete
/// marker: a deleted account keeps its row but
/// [`crate::ports::AccountStore::find`] returns `None` for it. The struct holds no
/// member list; membership is queried through the repo.
///
/// References: [`Account::open`], [`crate::ports::AccountStore`],
/// [`crate::ports::DidMinter`] (which mints `did`), DESIGN/Account, ZMVP-14.
pub struct Account {
    pub id: AccountId,
    pub did: Did,
    /// The public handle the account is reached by — a validated, normalized
    /// atproto handle chosen at founding (`POST /accounts`), unique across **all**
    /// accounts (a soft-deleted account still reserves its handle; DD/23003138).
    /// For a Zurfur-issued handle (`<label>.zurfur.app`) this is what the
    /// `/.well-known/atproto-did` resolver looks the account up by (ZMVP-44,
    /// DD/24870914 §6). See [`Handle`].
    pub handle: Handle,
    /// The name the founder gave the account. See [`AccountName`].
    pub name: AccountName,
    /// When the account was founded; equals `updated_at` at creation.
    pub created_at: DateTimeUtc,
    /// When the account was last changed.
    pub updated_at: DateTimeUtc,
    /// Soft-delete marker: `Some(when)` once deleted, else `None`.
    pub deleted_at: Option<DateTimeUtc>,
}

impl Account {
    /// Open an account and seat its founder as Owner — the ZMVP-14 invariant
    /// "the creating User becomes Owner".
    ///
    /// Mints the account (`AccountId::new(Uuid::now_v7())`, `created_at ==
    /// updated_at == now`) and pairs it with `UserAccount { user_id: owner,
    /// account_id: id, role: Role::Owner(None) }` — the founder seated as Owner
    /// with no role alias. The `name` and `handle` are already validated (see
    /// [`AccountName`], [`Handle`]); the `did` is minted upstream by a `DidMinter`.
    ///
    /// Named `open` ("open an account"), not `found`, to dodge the past tense of
    /// `find`.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{account::{Account, AccountName}, did::Did, handle::Handle, role::Role, user::UserId};
    ///
    /// let owner = UserId::new(uuid::Uuid::now_v7());
    /// let (account, membership) = Account::open(
    ///     owner,
    ///     Did::new("did:plc:example".to_string()),
    ///     Handle::try_new("acme.zurfur.app").unwrap(),
    ///     "Acme Studio".parse::<AccountName>().unwrap(),
    ///     Utc::now(),
    /// );
    /// assert_eq!(membership.role, Role::Owner(None)); // founder is Owner
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
            id: AccountId::new(uuid::Uuid::now_v7()),
            did,
            handle,
            name,
            created_at: now,
            updated_at: now,
            deleted_at: None,
        };
        let membership = UserAccount {
            user_id: owner,
            account_id: new_account.id,
            role: Role::Owner(None),
        };
        (new_account, membership)
    }
}

/// An account's public-facing profile: its [`Did`] and a display name.
///
/// The account analogue of a visitor's [`crate::elements::profile::Profile`] —
/// the public projection of an [`Account`], distinct from the private [`Account`]
/// row. See DESIGN/Account.
pub struct AccountProfile {
    pub did: Did,
    pub display_name: String,
}
