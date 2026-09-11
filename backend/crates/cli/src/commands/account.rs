//! The `account` namespace: what the acting identity does with Accounts.
//! `create` (ZMVP-205 slice 4) is the CLI face of
//! [`application::account::create_account`] — the same use case behind
//! `POST /accounts`, so both drivers found accounts through one path.

use std::path::Path;

use application::account::{
    AccountError, AccountPorts, CreateAccountCommand, CreateAccountResult, create_account,
};
use chrono::Utc;
use clap::Subcommand;
use composition::Runtime;
use domain::elements::{account::AccountName, handle::Handle};
use serde::Serialize;

use crate::{CliError, principal::Principal};

/// The account operations.
#[derive(Debug, Subcommand)]
pub enum AccountOp {
    /// Create (found) a new Account, with the acting identity as its Owner.
    Create {
        /// The account's display name.
        #[arg(long)]
        name: String,
        /// The account's handle — `<label>.zurfur.app` or a brought domain.
        #[arg(long)]
        handle: String,
    },
}

/// `create`'s projection — the same keys and spelling as the HTTP
/// `CreateAccountResponse` (`{id, did, handle, name}`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Founded {
    id: String,
    did: String,
    handle: String,
    name: String,
}

impl From<CreateAccountResult> for Founded {
    fn from(founded: CreateAccountResult) -> Self {
        Founded {
            id: founded.account_id.to_string(),
            did: founded.did.as_str().to_owned(),
            handle: founded.handle.as_str().to_owned(),
            name: founded.name.as_str().to_owned(),
        }
    }
}

/// Run one account op over the runtime as the identity at `identity_path`.
pub async fn run(
    runtime: &Runtime,
    identity_path: &Path,
    op: AccountOp,
) -> Result<serde_json::Value, CliError> {
    match op {
        AccountOp::Create { name, handle } => {
            // Founding is a write: resolve the principal first (the HTTP
            // driver's `require_user`), then parse — the same 422-class
            // refusals as the API's `invalid_request`, before anything is minted.
            let principal = Principal::resolve(runtime, identity_path).await?;
            let name = name
                .parse::<AccountName>()
                .map_err(|err| CliError::domain("invalid_request", err))?;
            let handle = handle
                .parse::<Handle>()
                .map_err(|err| CliError::domain("invalid_request", err))?;

            let command = CreateAccountCommand {
                actor: principal.user.id,
                name,
                handle,
            };
            let founded = create_account(
                command,
                account_ports(runtime),
                &runtime.config.handle_domain,
                Utc::now(),
            )
            .await
            .map_err(|err| match err {
                AccountError::HandleTaken => CliError::domain("handle_taken", err),
                // The terse `Display` is what the user sees; the cause goes to
                // the diagnostics channel (stderr, before the problem line).
                AccountError::Minter(_) => {
                    tracing::error!(error = ?err, "minting the account's did:plc failed");
                    CliError::infra("service_unavailable", err)
                }
                AccountError::Store(_) => {
                    tracing::error!(error = ?err, "founding the account failed in the store");
                    CliError::infra("internal_error", err)
                }
            })?;
            let body = Founded::from(founded);
            Ok(serde_json::to_value(body).expect("Founded serializes"))
        }
    }
}

/// The account use cases' ports, borrowed off the runtime bag — this
/// driver's copy of the API's `account_ports`.
fn account_ports(runtime: &Runtime) -> AccountPorts<'_> {
    AccountPorts {
        accounts: &*runtime.accounts,
        did_minter: &*runtime.did_minter,
        database: &*runtime.database,
    }
}
