//! Use cases about [`Account`]s.

use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::{Account, AccountId, AccountName},
        did::Did,
        handle::{Handle, HandleDomain},
        user::UserId,
    },
    ports::{AccountStore, Database, DidMinter, HandleTaken, UnitOfWork},
};
use shared::settings::HANDLE_QUARANTINE_WINDOW;

use crate::transaction;

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
    /// Minting the account's `did:plc` failed; nothing was persisted, the
    /// caller may retry.
    Minter(anyhow::Error),
    /// The account store failed.
    Store(anyhow::Error),
}

impl std::fmt::Display for AccountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountError::HandleTaken => write!(f, "{HandleTaken}"),
            AccountError::Minter(_) => write!(f, "the did minter failed"),
            AccountError::Store(_) => write!(f, "the account store failed"),
        }
    }
}

impl std::error::Error for AccountError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AccountError::HandleTaken => None,
            AccountError::Minter(e) | AccountError::Store(e) => Some(e.as_ref()),
        }
    }
}

/// The ports the account use cases reach: reads off [`AccountStore`], the
/// account's sovereign identity off [`DidMinter`], writes through a unit of
/// work vended by [`Database`]. Built by each driver off its runtime.
pub struct AccountPorts<'a> {
    pub accounts: &'a dyn AccountStore,
    pub did_minter: &'a dyn DidMinter,
    pub database: &'a dyn Database,
}

/// `create_account`'s input: the founder and the account's chosen name and
/// handle, both already validated by their newtypes at the driver's boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAccountCommand {
    /// The founder. Precondition: a user the store recognizes — the driver
    /// authenticated them (session, identity file). An unrecognized id fails
    /// the Owner membership write as [`AccountError::Store`], not a typed
    /// variant.
    pub actor: UserId,
    pub name: AccountName,
    pub handle: Handle,
}

/// The founded account, as the drivers render it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateAccountResult {
    pub account_id: AccountId,
    pub did: Did,
    pub handle: Handle,
    pub name: AccountName,
}

/// `POST /accounts`: found a new Account for `actor` and make them its Owner
/// (ZMVP-14). Per DESIGN/Account a user may own several accounts, so this
/// founds a fresh one on every call rather than being idempotent.
///
/// Order: the two handle pre-checks (live claim, then quarantine — only for
/// the Zurfur namespace, a BYO domain is the user's own DNS), then mint the
/// sovereign `did:plc` (fallible, key-generating — kept before any private
/// write so a failure persists nothing), then the account and the founder's
/// Owner membership commit together in one unit of work. The pre-checks can't
/// see a tombstoned reservation nor win a concurrent claim; the global unique
/// index surfaces both as [`HandleTaken`] at the write, mapped to
/// [`AccountError::HandleTaken`].
pub async fn create_account(
    command: CreateAccountCommand,
    ports: AccountPorts<'_>,
    handle_domain: &HandleDomain,
    now: DateTimeUtc,
) -> Result<CreateAccountResult, AccountError> {
    let live_claim = ports
        .accounts
        .find_did_by_handle(&command.handle)
        .await
        .map_err(AccountError::Store)?;
    if live_claim.is_some() {
        return Err(AccountError::HandleTaken);
    }

    if command.handle.is_in_namespace(handle_domain) {
        let quarantined = ports
            .accounts
            .handle_reserved_for_other(&command.handle, None, now - HANDLE_QUARANTINE_WINDOW)
            .await
            .map_err(AccountError::Store)?;
        if quarantined {
            return Err(AccountError::HandleTaken);
        }
    }

    let did = ports
        .did_minter
        .mint(&command.handle)
        .await
        .map_err(AccountError::Minter)?;

    let (account, owner) = Account::open(command.actor, did, command.handle, command.name, now);
    // The `async move` closure owns what it writes and hands the committed
    // account back out for the result.
    let account = transaction(ports.database, async move |uow: &mut dyn UnitOfWork| {
        uow.accounts().create(&account, &owner).await?;
        Ok(account)
    })
    .await
    .map_err(|err| {
        if err.downcast_ref::<HandleTaken>().is_some() {
            AccountError::HandleTaken
        } else {
            AccountError::Store(err)
        }
    })?;

    Ok(CreateAccountResult {
        account_id: account.id,
        did: account.did,
        handle: account.handle,
        name: account.name,
    })
}
