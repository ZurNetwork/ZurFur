//! The `account` namespace: what the acting identity does with Accounts.
//! `create` is the CLI face of [`application::account::Accounts::create`]
//! (`POST /accounts`); `delete` mirrors
//! [`application::account::Accounts::delete`] (`DELETE /accounts/{id}`),
//! plus one thing HTTP has no place for: it asks first (`crate::confirm`).

use std::path::Path;

use application::account::{self, AccountEntity, AccountError, delete::DeleteOutcome};
use chrono::Utc;
use clap::Subcommand;
use composition::Runtime;
use domain::elements::{
    account::{AccountId, AccountName},
    handle::Handle,
};
use serde::Serialize;

use crate::{CliError, confirm::confirm_destructive, principal::Principal};

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
    /// Delete an Account the acting identity owns — soft if it holds facts,
    /// hard if empty. Asks to confirm first.
    Delete {
        /// The account's id — its did:plc (DD 57081857).
        account_id: AccountId,
        /// Skip the confirmation prompt (for scripts).
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

/// `create`'s projection — the same keys as the HTTP `CreateAccountResponse`
/// (`{id, did, handle, name}`). `id` and `did` carry the same value (DD
/// 57081857 folded the surrogate id into the sovereign DID).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Founded {
    id: String,
    did: String,
    handle: String,
    name: String,
}

impl From<account::create::Output> for Founded {
    fn from(founded: account::create::Output) -> Self {
        let did = founded.account_id.to_string();
        Founded {
            id: did.clone(),
            did,
            handle: founded.handle.as_str().to_owned(),
            name: founded.name.as_str().to_owned(),
        }
    }
}

/// `delete`'s projection — the same key as HTTP's `DeleteAccountResponse`
/// (`{outcome}`), which the CLI cannot import (it lives behind axum in
/// `api`). Pinned to the wire by `api/tests/delete_account_parity.rs`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Deleted {
    outcome: String,
}

impl From<account::delete::Output> for Deleted {
    fn from(deleted: account::delete::Output) -> Self {
        let outcome = match deleted.outcome {
            DeleteOutcome::Soft => "soft",
            DeleteOutcome::Hard => "hard",
        };
        Deleted {
            outcome: outcome.to_owned(),
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
            // Resolve the principal first, then parse — nothing is minted before both succeed.
            let principal = Principal::resolve(runtime, identity_path).await?;
            let name = name
                .parse::<AccountName>()
                .map_err(|err| CliError::domain("invalid_request", err))?;
            let handle = handle
                .parse::<Handle>()
                .map_err(|err| CliError::domain("invalid_request", err))?;

            let command = account::create::Command {
                actor_id: principal.user.id,
                name,
                handle,
            };
            let founded = runtime
                .app()
                .accounts()
                .create(command, &runtime.config.handle_domain, Utc::now())
                .await
                .map_err(|err| match err {
                    AccountError::HandleTaken => CliError::domain("handle_taken", err),
                    // The terse `Display` is what the user sees; the cause goes to stderr.
                    AccountError::Infrastructure(_) => {
                        tracing::error!(error = ?err, "founding the account failed");
                        CliError::infra("internal_error", err)
                    }
                    // Unreachable from founding; mapped to keep the match exhaustive.
                    AccountError::IncorrectRole => CliError::domain("forbidden", err),
                    AccountError::NotFound(AccountEntity::Account) => {
                        CliError::domain("account_not_found", err)
                    }
                    // Unreachable from founding; mapped to keep the match exhaustive.
                    AccountError::NotFound(_) => CliError::domain("not_found", err),
                    // Unreachable from founding — the namespace check is change_handle's own.
                    AccountError::UnsupportedHandle => CliError::domain("unsupported_handle", err),
                    // Unreachable from founding — both are change_handle-only outcomes.
                    AccountError::HandleUnchanged => CliError::domain("invalid_request", err),
                    AccountError::RenamedTooRecently => CliError::domain("rate_limited", err),
                    // Unreachable here: membership-only outcomes of accept/leave.
                    AccountError::NoPendingInvitation => {
                        CliError::domain("no_pending_invitation", err)
                    }
                    AccountError::NotAMember => CliError::domain("member_not_found", err),
                    AccountError::OwnerCannotLeave => CliError::domain("owner_cannot_leave", err),
                    AccountError::IncorrectTransferOfAccount => CliError::domain("forbidden", err),
                    AccountError::DidBelongsToAnotherActor => {
                        CliError::domain("did_belongs_to_another_actor", err)
                    }
                    // Unreachable here: membership-only outcomes of invite/transfer.
                    AccountError::AlreadyMember => CliError::domain("already_member", err),
                    AccountError::CannotTransferToSelf => CliError::domain("invalid_request", err),
                    // Unreachable from founding — no invitation is issued here.
                    AccountError::InvitationAlreadyPending => {
                        CliError::domain("invalid_request", err)
                    }
                    // Unreachable from founding; mapped to keep the match exhaustive.
                    AccountError::ContainsCommissions
                    | AccountError::DuplicateName
                    | AccountError::IncorrectNumberOfColumns
                    | AccountError::NothingToDo
                    | AccountError::IndexOutOfRange(_) => CliError::domain("invalid_request", err),
                    AccountError::SystemError(_) => CliError::domain("internal_error", err),
                })?;
            let body = Founded::from(founded);
            Ok(serde_json::to_value(body).expect("Founded serializes"))
        }
        AccountOp::Delete { account_id, yes } => {
            // Resolve the principal first, so an unrecognized caller is turned away
            // before any account is loaded. `account_id` is already parsed (clap).
            let principal = Principal::resolve(runtime, identity_path).await?;
            // Confirm only after resolving, and act only after confirming — a
            // declined prompt leaks nothing about whether the id names anything.
            if !yes {
                let account_id_display = account_id.to_string();
                let prompt = format!(
                    "Delete account {account_id_display}? This frees its handle and \
                     tombstones its did:plc; it cannot be undone after the PLC recovery \
                     window. [y/N] "
                );
                confirm_destructive(&prompt)?;
            }
            let command = account::delete::Command {
                actor_id: principal.user.id,
                account_id,
            };
            let deleted =
                runtime
                    .app()
                    .accounts()
                    .delete(command)
                    .await
                    .map_err(|err| match err {
                        AccountError::NotFound(AccountEntity::Account) => {
                            CliError::domain("account_not_found", err)
                        }
                        // Unreachable from delete; mapped to keep the match exhaustive.
                        AccountError::NotFound(_) => CliError::domain("not_found", err),
                        AccountError::IncorrectRole => CliError::domain("forbidden", err),
                        // The terse `Display` is what the user sees; the cause goes to stderr.
                        AccountError::Infrastructure(_) => {
                            tracing::error!(error = ?err, "deleting the account failed");
                            CliError::infra("internal_error", err)
                        }
                        // Unreachable from delete (no handle is claimed here).
                        AccountError::HandleTaken => CliError::domain("handle_taken", err),
                        // Unreachable from delete — no handle-namespace check runs here.
                        AccountError::UnsupportedHandle => {
                            CliError::domain("unsupported_handle", err)
                        }
                        // Unreachable from delete — both are change_handle-only outcomes.
                        AccountError::HandleUnchanged => CliError::domain("invalid_request", err),
                        AccountError::RenamedTooRecently => CliError::domain("rate_limited", err),
                        // Unreachable here: membership-only outcomes of accept/leave.
                        AccountError::NoPendingInvitation => {
                            CliError::domain("no_pending_invitation", err)
                        }
                        AccountError::NotAMember => CliError::domain("member_not_found", err),
                        AccountError::OwnerCannotLeave => {
                            CliError::domain("owner_cannot_leave", err)
                        }
                        AccountError::IncorrectTransferOfAccount => {
                            CliError::domain("forbidden", err)
                        }
                        AccountError::DidBelongsToAnotherActor => {
                            CliError::domain("did_belongs_to_another_actor", err)
                        }
                        // Unreachable here: membership-only outcomes of invite/transfer.
                        AccountError::AlreadyMember => CliError::domain("already_member", err),
                        AccountError::CannotTransferToSelf => {
                            CliError::domain("invalid_request", err)
                        }
                        // Unreachable from delete — no invitation is issued here.
                        AccountError::InvitationAlreadyPending => {
                            CliError::domain("invalid_request", err)
                        }
                        // Unreachable from delete; mapped to keep the match exhaustive.
                        AccountError::ContainsCommissions
                        | AccountError::DuplicateName
                        | AccountError::IncorrectNumberOfColumns
                        | AccountError::NothingToDo
                        | AccountError::IndexOutOfRange(_) => {
                            CliError::domain("invalid_request", err)
                        }
                        AccountError::SystemError(_) => CliError::domain("internal_error", err),
                    })?;
            let body = Deleted::from(deleted);
            Ok(serde_json::to_value(body).expect("Deleted serializes"))
        }
    }
}
