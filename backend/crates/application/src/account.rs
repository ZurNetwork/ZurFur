//! Use cases about [`Account`]s.

use domain::{
    elements::{
        account::{Account, AccountId},
        workflow::WorkflowError,
    },
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
pub mod workflow;
/// Account use cases, with the ports already bound. A namespace, not a
/// mediator: one `impl Accounts<'_>` block per use-case file.
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

    /// Cannot fail: [`Ports`](crate::Ports) always carries a DID minter.
    fn try_from(ports: &'a crate::Ports) -> Result<Self, Self::Error> {
        let did_minter = ports.did_minter.as_ref();
        Ok(Self::new(ports, did_minter))
    }
}

impl<'a> From<&'a crate::App> for Accounts<'a> {
    /// Binds the namespace to the app's ports.
    fn from(app: &'a crate::App) -> Self {
        Self::try_from(app.ports()).expect("composition root supplies the DID minter")
    }
}

#[derive(Debug)]
pub enum AccountEntity {
    Account,
    User,
    Workflow,
    Column,
    Commission,
}

/// Why an account use case could not answer. `Display` stays terse and never
/// interpolates the cause; the cause rides
/// [`source`](std::error::Error::source).
#[derive(Debug)]
pub enum AccountError {
    /// The handle is claimed — live, tombstoned or quarantined (DD 23003138).
    HandleTaken,
    /// The handle's namespace isn't supported for this operation.
    UnsupportedHandle,
    /// A port failed; nothing was persisted, the caller may retry.
    Infrastructure(anyhow::Error),
    /// The actor's role doesn't carry the needed authority — no role included.
    IncorrectRole,
    /// The target handle is already the account's current one.
    HandleUnchanged,
    /// The account has changed its handle too often recently.
    RenamedTooRecently,
    /// The actor holds no pending invitation into the account.
    NoPendingInvitation,
    /// The actor holds no membership in the account.
    NotAMember,
    /// The Owner can't leave while still Owner — transfer or delete first.
    OwnerCannotLeave,
    IncorrectTransferOfAccount,
    /// The grantee's DID is interned as a non-User actor.
    DidBelongsToAnotherActor,
    AlreadyMember,
    CannotTransferToSelf,
    InvitationAlreadyPending,
    ContainsCommissions,
    DuplicateName,
    NotFound(AccountEntity),
    SystemError(SystemError),
    IncorrectNumberOfColumns,
    NothingToDo,
    /// The requested position is past the end of the list; carries the
    /// offending index.
    IndexOutOfRange(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemError(Option<&'static str>);

impl std::error::Error for SystemError {}

impl std::fmt::Display for SystemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let SystemError(e) = self;
        match e {
            Some(err) => write!(f, "{err}"),
            None => write!(f, "unspecified system error"),
        }
    }
}
impl From<&'static str> for SystemError {
    fn from(value: &'static str) -> Self {
        Self(Some(value))
    }
}

impl AccountError {
    pub fn sys_err(s: &'static str) -> AccountError {
        Self::SystemError(SystemError::from(s))
    }
}

impl From<WorkflowError> for AccountError {
    fn from(value: WorkflowError) -> Self {
        match value {
            WorkflowError::DuplicateColumnName => AccountError::DuplicateName,
            WorkflowError::IndexOutOfRange(index) => AccountError::IndexOutOfRange(index),
            v => AccountError::Infrastructure(v.into()),
        }
    }
}

impl std::fmt::Display for AccountEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Account => write!(f, "account"),
            Self::User => write!(f, "user"),
            Self::Column => write!(f, "column"),
            Self::Workflow => write!(f, "workflow"),
            Self::Commission => write!(f, "commission"),
        }
    }
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
            Self::InvitationAlreadyPending => write!(f, "An invitation was already pending"),
            Self::ContainsCommissions => write!(f, "Contains commissions"),
            Self::DuplicateName => write!(f, "Duplicate name"),
            Self::NotFound(entity) => write!(f, "{entity} not found"),
            Self::SystemError(error) => match error.0 {
                Some(e) => write!(f, "system error: {e}"),
                None => {
                    write!(f, "there was a system error while processing this request")
                }
            },
            Self::IncorrectNumberOfColumns => {
                write!(f, "The number of columns provided is erroneous")
            }
            Self::NothingToDo => write!(f, "nothing to do"),
            Self::IndexOutOfRange(index) => {
                write!(f, "position {index} is past the end of the list")
            }
        }
    }
}

impl std::error::Error for AccountError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AccountError::HandleTaken
            | AccountError::UnsupportedHandle
            | AccountError::IncorrectRole
            | AccountError::RenamedTooRecently
            | AccountError::HandleUnchanged
            | AccountError::NoPendingInvitation
            | AccountError::NotAMember
            | AccountError::OwnerCannotLeave
            | AccountError::IncorrectTransferOfAccount
            | AccountError::DidBelongsToAnotherActor
            | AccountError::AlreadyMember
            | AccountError::CannotTransferToSelf
            | Self::ContainsCommissions
            | Self::InvitationAlreadyPending
            | Self::IncorrectNumberOfColumns
            | Self::NothingToDo
            | Self::IndexOutOfRange(_)
            | Self::DuplicateName => None,
            Self::NotFound(entity) => Some(entity),
            Self::SystemError(e) => Some(e),
            AccountError::Infrastructure(e) => Some(e.as_ref()),
        }
    }
}

impl std::error::Error for AccountEntity {}

/// The one place a store error becomes a use-case error: [`HandleTaken`] and
/// [`DidBelongsToAnotherActor`] are recognized as state conflicts, anything
/// else stays [`Infrastructure`](AccountError::Infrastructure).
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

/// Resolve the account a use case acts on, or refuse with `NotFound` — the one
/// liveness gate every account use case shares. An unknown id and a
/// soft-deleted account get the same answer (DD 23003138).
pub(crate) async fn require_live_account(
    ports: &crate::Ports,
    account_id: &AccountId,
) -> AccountResult<Account> {
    // Existence before standing: `role_of` carries no tombstone predicate, so
    // a use case starting there would act on soft-deleted accounts.
    ports
        .accounts
        .find(account_id)
        .await?
        .ok_or(AccountError::NotFound(AccountEntity::Account))
}

/// The ports the account use cases reach: reads off [`AccountStore`] and
/// [`UserStore`], identity off [`DidMinter`], writes through a unit of work
/// vended by [`Database`].
pub struct AccountPorts<'a> {
    pub accounts: &'a dyn AccountStore,
    pub users: &'a dyn UserStore,
    pub did_minter: &'a dyn DidMinter,
    pub database: &'a dyn Database,
}

pub type AccountResult<T> = Result<T, AccountError>;
