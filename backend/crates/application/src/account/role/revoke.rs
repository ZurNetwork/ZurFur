use crate::LazyUnit;
use crate::use_case;
use domain::elements::{account::AccountId, user::UserId};

use crate::{
    Ports,
    account::{
        AccountError, AccountResult, ActorLock, lock_actor_and_target, lock_live_account,
        refused_after_a_race, require_live_account, role::Roles,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub account_id: AccountId,
    pub target_id: UserId,
    pub actor_id: UserId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub account_id: AccountId,
    pub user_id: UserId,
}

impl Roles<'_> {
    #[use_case]
    pub async fn revoke(
        &self,
        #[ports] ports: &Ports,
        #[lazy_unit] uow: &mut LazyUnit<'_>,
        cmd: Command,
    ) -> AccountResult<Output> {
        let Command {
            account_id,
            target_id,
            actor_id,
        } = cmd;

        require_live_account(ports, &account_id).await?;

        // The actor's own standing is settled BEFORE the target's membership is
        // looked at, as `grant` already does. The order is load-bearing: the two
        // refusals below are told apart on the wire (`404 member_not_found` vs
        // `403 forbidden`), so probing the target first would answer "is X a
        // member of this account?" for any signed-in caller, member or not.
        let actor_role = ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .ok_or(AccountError::IncorrectRole)?;

        // A target who holds no role here is a *missing membership*, not a
        // refusal — every sibling use case (`leave`, `transfer_ownership`,
        // `invitation::revoke`) already answers `NotAMember`. This one answered
        // `UserNotFound`, which the driver renders `403`: a revoke of someone
        // who was never a member reported as "you may not", not "there is no
        // such member".
        let target_role = ports
            .accounts
            .role_of(&target_id, &account_id)
            .await?
            .ok_or(AccountError::NotAMember)?;

        if !actor_role.can_grant(&target_role) {
            return Err(AccountError::IncorrectRole);
        }

        let uow = uow.open().await?;
        lock_live_account(uow, &account_id).await?;
        // Decided again on the locked rows, the actor's standing first as above: a
        // demotion of the actor, the target's departure or a transfer to the target may
        // have landed since.
        let (actor_now, target_now) = lock_actor_and_target(
            &mut *uow.accounts(),
            &account_id,
            &actor_id,
            ActorLock::Share,
            &target_id,
        )
        .await?;
        let actor_now =
            actor_now.ok_or_else(|| refused_after_a_race("revoke", AccountError::IncorrectRole))?;
        let target_now =
            target_now.ok_or_else(|| refused_after_a_race("revoke", AccountError::NotAMember))?;
        if !actor_now.can_grant(&target_now) {
            return Err(refused_after_a_race("revoke", AccountError::IncorrectRole));
        }
        uow.accounts().revoke_role(&target_id, &account_id).await?;

        Ok(Output {
            account_id,
            user_id: target_id,
        })
    }
}
