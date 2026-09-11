use domain::elements::{account::AccountId, user::UserId};

use crate::{
    account::{AccountError, AccountResult, invitation::Invitations},
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub account_id: AccountId,
    pub actor_id: UserId,
    pub target_id: UserId,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output;

impl Invitations<'_> {
    /// Revokes the target's pending invitation into the account. The actor must
    /// hold a role that outranks the offered one; anything less is `NotAMember`.
    /// Answers `AlreadyMember` when the target already holds a role, and
    /// `NoPendingInvitation` when no live offer exists.
    pub async fn revoke(&self, cmd: Command) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            account_id,
            actor_id,
            target_id,
        } = cmd;

        // The actor's own standing is settled BEFORE the target is looked at,
        // as `role::revoke` already does. The order is load-bearing: the three
        // refusals below are told apart on the wire (`409 already_member` vs
        // `404 no_pending_invitation` vs `404 member_not_found`), so probing
        // the target first answered "is this DID a member of — or invited to —
        // this account?" for any signed-in caller, member or not. A pending
        // invitation is not public.
        let actor_role = ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .ok_or(AccountError::NotAMember)?;

        if ports
            .accounts
            .role_of(&target_id, &account_id)
            .await?
            .is_some()
        {
            return Err(AccountError::AlreadyMember);
        };

        let invitation = ports
            .accounts
            .find_pending_invitation(&account_id, &target_id)
            .await?
            .ok_or(AccountError::NoPendingInvitation)?;

        // The rank half of the same gate, re-checked once the offer names its
        // role — standing alone is not authority over an offer that outranks
        // the actor. Same refusal as holding no standing at all, so the two are
        // one answer on the wire.
        if !actor_role.can_grant(&invitation.role) {
            return Err(AccountError::NotAMember);
        }

        let mut uow = self.ports().database.begin().await?;
        uow.accounts().revoke_invitation(&invitation.id).await?;
        uow.commit().await?;
        Ok(Output)
    }
}
