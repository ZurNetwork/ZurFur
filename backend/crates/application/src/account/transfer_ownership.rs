use crate::LazyUnit;
use crate::use_case;
use domain::elements::{account::AccountId, role::Role, user::UserId};

use crate::{
    Ports,
    account::{
        AccountError, AccountResult, Accounts, ActorLock, lock_actor_and_target, lock_live_account,
        refused_after_a_race, require_live_account,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub account_id: AccountId,
    pub actor_id: UserId,
    pub target_id: UserId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub account_id: AccountId,
    pub owner_id: UserId,
    pub previous_owner_id: UserId,
}

impl Accounts<'_> {
    #[use_case]
    pub async fn transfer_ownership(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
    ) -> AccountResult<Output> {
        let Command {
            account_id,
            actor_id,
            target_id,
        } = cmd;

        if actor_id == target_id {
            return Err(AccountError::CannotTransferToSelf);
        }

        require_live_account(ports, &account_id).await?;

        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|role| matches!(role, Role::Owner))
            .ok_or(AccountError::IncorrectRole)?;

        ports
            .accounts
            .role_of(&target_id, &account_id)
            .await?
            .ok_or(AccountError::NotAMember)?;

        let uow = uow.open().await?;
        lock_live_account(uow, &account_id).await?;
        // Decided again on the locked rows, the actor's standing first as above; the
        // actor's row is locked for update, since the transfer demotes them.
        let (actor_now, heir_now) = lock_actor_and_target(
            &mut *uow.accounts(),
            &account_id,
            &actor_id,
            ActorLock::Update,
            &target_id,
        )
        .await?;
        if actor_now != Some(Role::Owner) {
            return Err(refused_after_a_race(
                "transfer_ownership",
                AccountError::IncorrectRole,
            ));
        }
        if heir_now.is_none() {
            return Err(refused_after_a_race(
                "transfer_ownership",
                AccountError::NotAMember,
            ));
        }
        uow.accounts()
            .transfer_ownership(&actor_id, &target_id, &account_id)
            .await?;
        Ok(Output {
            account_id,
            owner_id: target_id,
            previous_owner_id: actor_id,
        })
    }
}
