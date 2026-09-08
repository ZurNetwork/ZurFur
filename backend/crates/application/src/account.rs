//! Use cases about [`Account`]s.

use domain::{
    elements::account::{Account, AccountId},
    ports::{AccountStore, Database, DidBelongsToAnotherActor, DidMinter, HandleTaken, UserStore},
};

use crate::ports::WithPorts;

pub mod change_handle;
pub mod create;
pub mod delete;
pub mod facts;
pub mod invitation;
pub mod leave;
pub mod list;
pub mod role;
pub mod transfer_ownership;
/// Account use cases, with the ports already bound. A namespace, not a
/// mediator: one `impl Accounts<'_>` block per use-case file, one use case each.
#[derive(Clone, Copy)]
pub struct Accounts<'a> {
    ports: &'a crate::Ports,
    did_minter: &'a dyn DidMinter,
}

impl<'a> WithPorts<'a> for Accounts<'a> {
    fn ports(&self) -> &'a crate::Ports {
        self.ports
    }
}

impl<'a> Accounts<'a> {
    /// Bind the namespace to resolved dependencies.
    pub fn new(ports: &'a crate::Ports, did_minter: &'a dyn DidMinter) -> Self {
        Self { ports, did_minter }
    }

    /// The bag this namespace was built over.
    pub fn ports(&self) -> &'a crate::Ports {
        self.ports
    }

    /// The did:plc minter.
    pub fn did_minter(&self) -> &'a dyn DidMinter {
        self.did_minter
    }
}

impl<'a> TryFrom<&'a crate::Ports> for Accounts<'a> {
    type Error = crate::MissingPort;

    /// Cannot fail: [`Ports`](crate::Ports) carries a DID minter unconditionally,
    /// so [`MissingPort`](crate::MissingPort) is unreachable from here.
    fn try_from(ports: &'a crate::Ports) -> Result<Self, Self::Error> {
        let did_minter = ports.did_minter.as_ref();
        Ok(Self::new(ports, did_minter))
    }
}

impl<'a> From<&'a crate::App> for Accounts<'a> {
    /// Binds the namespace to the app's ports. The `expect` is unreachable while
    /// [`try_from`](Accounts::try_from) is infallible.
    fn from(app: &'a crate::App) -> Self {
        Self::try_from(app.ports()).expect("composition root supplies the DID minter")
    }
}

/// Why an account use case could not answer. One enum per module: a driver
/// maps each variant to its own surface (problem+json, `{class, code}`).
///
/// `Display` is deliberately terse and never interpolates the cause — a
/// store error can carry SQL, constraint names or custody paths, and a
/// driver printing `{err}` must not leak them. The cause stays on
/// [`source`](std::error::Error::source) for tracing.
#[derive(Debug)]
pub enum AccountError {
    /// The handle is claimed — by a live account, a tombstoned one (the
    /// global unique index, DD `23003138`), or quarantined to the account
    /// that vacated it (DD `27852802` §4).
    HandleTaken,
    /// The handle's namespace isn't supported for this operation — e.g.
    /// changing *to* a brought (BYO) domain, deferred until bidirectional
    /// verify-before-commit ships (DD `27852802` §6).
    UnsupportedHandle,
    /// A port (the did:plc minter, the account store) failed; nothing was
    /// persisted, the caller may retry.
    Infrastructure(anyhow::Error),
    /// The actor's role on the account doesn't carry the authority this use
    /// case needs — including holding no role at all (a non-member).
    IncorrectRole,
    /// The account named by the command is not a live account: unknown, or
    /// already soft-deleted (DD `23003138`). This may also refer to a non-existent user.
    AccountNotFound,
    /// The target handle is already the account's current one.
    HandleUnchanged,
    /// The account has changed its handle too often recently (DD `27852802` §3).
    RenamedTooRecently,
    /// The actor holds no pending invitation into the account.
    NoPendingInvitation,
    /// The actor holds no membership in the account.
    NotAMember,
    /// The Owner can't leave while still Owner — transfer or delete first.
    OwnerCannotLeave,
    IncorrectTransferOfAccount,
    /// The grantee's DID is interned as a non-User actor (an Account, a Character).
    DidBelongsToAnotherActor,
    AlreadyMember,
    CannotTransferToSelf,
    UserNotFound,
    InvitationAlreadyPending,
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountError::HandleTaken => write!(f, "{HandleTaken}"),
            AccountError::UnsupportedHandle => write!(
                f,
                "This handle's namespace isn't supported for this operation"
            ),
            AccountError::Infrastructure(_) => write!(f, "an infrastructure port failed"),
            AccountError::IncorrectRole => write!(f, "This role can't perform this action"),
            AccountError::AccountNotFound => write!(f, "This account does not exist"),
            AccountError::HandleUnchanged => write!(f, "Nothing to do"),
            AccountError::RenamedTooRecently => write!(f, "Rate limited"),
            AccountError::NoPendingInvitation => write!(f, "No pending invitation"),
            AccountError::NotAMember => write!(f, "Not a member of this account"),
            AccountError::OwnerCannotLeave => {
                write!(
                    f,
                    "The Owner can't leave; transfer or delete the account first"
                )
            }
            AccountError::IncorrectTransferOfAccount => {
                write!(f, "Owner can't be granted; transfer ownership instead")
            }
            AccountError::DidBelongsToAnotherActor => {
                write!(f, "That DID belongs to another kind of actor")
            }
            AccountError::AlreadyMember => write!(f, "Already a member"),
            AccountError::CannotTransferToSelf => write!(f, "Already the owner"),
            Self::UserNotFound => write!(f, "User not found!"),
            Self::InvitationAlreadyPending => write!(f, "An invitation was already pending"),
        }
    }
}

impl std::error::Error for AccountError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AccountError::HandleTaken
            | AccountError::UnsupportedHandle
            | AccountError::IncorrectRole
            | AccountError::AccountNotFound
            | AccountError::RenamedTooRecently
            | AccountError::HandleUnchanged
            | AccountError::NoPendingInvitation
            | AccountError::NotAMember
            | AccountError::OwnerCannotLeave
            | AccountError::IncorrectTransferOfAccount
            | AccountError::DidBelongsToAnotherActor
            | AccountError::AlreadyMember
            | AccountError::CannotTransferToSelf
            | Self::InvitationAlreadyPending
            | AccountError::UserNotFound => None,
            AccountError::Infrastructure(e) => Some(e.as_ref()),
        }
    }
}

/// The **one** place a store error becomes a use-case error.
///
/// Two store errors are typed precisely because the wire owes them a precise
/// answer: [`HandleTaken`] is the store's own backstop against a handle that a
/// pre-flight read could not see was spoken for (a soft-deleted account still
/// holds its handle), and [`DidBelongsToAnotherActor`] is a DID already interned
/// as a different kind of actor. Both are `409` state conflicts, and both
/// degrade into a `500` if they are swallowed as infrastructure. Doing the
/// translation here, on the `?` path every use case takes, is what keeps that
/// from happening one call site at a time.
impl From<anyhow::Error> for AccountError {
    fn from(err: anyhow::Error) -> Self {
        if err.downcast_ref::<HandleTaken>().is_some() {
            Self::HandleTaken
        } else if err.downcast_ref::<DidBelongsToAnotherActor>().is_some() {
            Self::DidBelongsToAnotherActor
        } else {
            Self::Infrastructure(err)
        }
    }
}

/// Resolve the account a use case acts on, or refuse with
/// [`AccountNotFound`](AccountError::AccountNotFound) — the one liveness gate
/// every account use case shares. An unknown id and a soft-deleted account get
/// the same answer (DD `23003138`).
pub(crate) async fn require_live_account(
    ports: &crate::Ports,
    account_id: &AccountId,
) -> AccountResult<Account> {
    // Existence before standing, and before any `role_of`: `role_of` reads the
    // membership table alone, with no tombstone predicate, so a use case that
    // starts there acts on soft-deleted accounts — an invitation could be
    // issued into, and accepted on, an account that is gone. Answering here
    // also keeps an act aimed at an account that is not there from coming back
    // `403`, a refusal implying there is something to be refused (see
    // `delete`). Lives here, once, because per-use-case copies drift — they
    // already had, three ways.
    ports
        .accounts
        .find(account_id)
        .await?
        .ok_or(AccountError::AccountNotFound)
}

/// The ports the account use cases reach: reads off [`AccountStore`], the
/// account's sovereign identity off [`DidMinter`], writes through a unit of
/// work vended by [`Database`]. Built by each driver off its runtime.
pub struct AccountPorts<'a> {
    pub accounts: &'a dyn AccountStore,
    pub users: &'a dyn UserStore,
    pub did_minter: &'a dyn DidMinter,
    pub database: &'a dyn Database,
}

pub type AccountResult<T> = Result<T, AccountError>;
