use domain::elements::{account::AccountId, role::Role, user::UserId};

use crate::account::{AccountError, AccountResult, Accounts, facts, require_live_account};

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
    pub async fn delete(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            account_id,
            actor_id,
        } = cmd;

        // Existence before standing: otherwise a delete aimed at an account
        // that is not there answers `403`.
        require_live_account(ports, &account_id).await?;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|role| matches!(role, Role::Owner))
            .ok_or(AccountError::IncorrectRole)?;
        // FIXME: Add facts in the correct way here
        let existing_facts = facts::exist::Query {
            account_id: account_id.clone(),
        };

        let mut uow = ports.database.begin().await?;
        let outcome = if self.facts().exist(existing_facts).await?.has_facts {
            uow.accounts().soft_delete(&account_id).await?;
            DeleteOutcome::Soft
        } else {
            uow.accounts().hard_delete(&account_id).await?;
            if let Err(err) = ports.did_minter.tombstone(&account_id).await {
                tracing::warn!(
                    error = ?err,
                    did = %account_id.as_str(),
                    "did:plc tombstone failed after hard delete; the PLC recovery window still applies"
                )
            };
            DeleteOutcome::Hard
        };
        uow.commit().await?;
        Ok(Output { outcome })
    }
}
