use crate::LazyUnit;
use crate::use_case;
use domain::elements::{account::AccountId, role::Role, user::UserId};

use crate::{
    Ports,
    account::{AccountEntity, AccountError, AccountResult, Accounts, require_live_account},
};

pub struct Command {
    pub actor_id: UserId,
    pub account_id: AccountId,
}
pub enum DeleteOutcome {
    Soft,
    Hard,
}

impl std::fmt::Display for DeleteOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Soft => write!(f, "soft"),
            Self::Hard => write!(f, "hard"),
        }
    }
}
pub struct Output {
    pub outcome: DeleteOutcome,
}

impl<'a> Accounts<'a> {
    /// Deletes the account, then, once that delete has committed, tombstones the DID of a
    /// hard-deleted one. A tombstone failure is logged, its cause at debug only, and the delete stands.
    #[use_case]
    pub async fn delete(&self, #[ports] ports: &Ports, cmd: Command) -> AccountResult<Output> {
        let account_id = cmd.account_id.clone();
        let removed = self.remove(cmd).await?;
        if let DeleteOutcome::Hard = removed.outcome
            && let Err(err) = ports.did_minter.tombstone(account_id.did()).await
        {
            tracing::warn!(
                "did:plc tombstone failed after hard delete; the PLC recovery window still applies"
            );
            tracing::debug!(error = %err, "the did:plc tombstone's failure");
        };
        Ok(removed)
    }

    /// Removes the account in one unit: it locks the Account row, re-reads the actor's
    /// role and the Account's facts there, then deletes soft when it holds a fact and hard
    /// otherwise. The unit commits when this returns `Ok`.
    #[use_case]
    async fn remove(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
    ) -> AccountResult<Output> {
        let Command {
            account_id,
            actor_id,
        } = cmd;

        // Existence before standing: otherwise a delete aimed at an account
        // that is not there answers `403`.
        require_live_account(ports, &account_id).await?;
        require_owner(ports.accounts.role_of(&actor_id, &account_id).await?)?;

        let uow = uow.open().await?;
        uow.accounts()
            .find_for_update(&account_id)
            .await?
            .ok_or(AccountError::NotFound(AccountEntity::Account))?;
        // The pool's answer may predate a hand-over that committed while the lock waited.
        require_owner(uow.accounts().role_of(&actor_id, &account_id).await?)?;
        let facts = uow.accounts().facts(&account_id).await?;

        let outcome = if facts.is_empty() {
            uow.accounts().hard_delete(&account_id).await?;
            DeleteOutcome::Hard
        } else {
            uow.accounts().soft_delete(&account_id).await?;
            DeleteOutcome::Soft
        };
        Ok(Output { outcome })
    }
}

/// Refuses with [`IncorrectRole`](AccountError::IncorrectRole) unless `role` is Owner.
fn require_owner(role: Option<Role>) -> AccountResult<()> {
    role.filter(|role| matches!(role, Role::Owner))
        .map(|_| ())
        .ok_or(AccountError::IncorrectRole)
}
