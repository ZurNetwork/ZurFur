use domain::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId,
        invitation::{Invitation, InvitationId, InvitationState},
        role::Role,
        user::UserId,
    },
};

use crate::{
    account::{AccountError, AccountResult, invitation::Invitations, require_live_account},
    ports::WithPorts,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub account_id: AccountId,
    pub target_id: UserId,
    pub actor_id: UserId,
    pub role: Role,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InviteOutcome {
    Minted,
    AlreadyPending,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Output {
    pub outcome: InviteOutcome,
    pub invitation_id: InvitationId,
    pub account_id: AccountId,
    pub role: Role,
    pub state: InvitationState,
    pub target_id: UserId,
}

impl Invitations<'_> {
    /// Issues a pending invitation into the account at `role`; a live offer for
    /// the same user is returned as-is rather than duplicated. The actor must
    /// hold a role that outranks the offered one. Answers `AccountNotFound` for
    /// a dead account, `IncorrectRole` without the rank, `AlreadyMember` when
    /// the invitee already holds one.
    pub async fn issue(&self, cmd: Command, now: DateTimeUtc) -> AccountResult<Output> {
        let ports = self.ports();
        let Command {
            account_id,
            target_id,
            actor_id,
            role,
        } = cmd;
        require_live_account(ports, &account_id).await?;
        ports
            .accounts
            .role_of(&actor_id, &account_id)
            .await?
            .filter(|r| r.can_grant(&role))
            .ok_or(AccountError::IncorrectRole)?;
        let mut uow = self.ports().database.begin().await?;
        let target = uow.users().provision(&target_id).await?;
        if ports
            .accounts
            .role_of(&target.id, &account_id)
            .await?
            .is_some()
        {
            return Err(AccountError::AlreadyMember);
        };

        if let Some(existing_invitation) = ports
            .accounts
            .find_pending_invitation(&account_id, &target.id)
            .await?
        {
            uow.commit().await?;
            return Ok(Output {
                account_id,
                invitation_id: existing_invitation.id,
                outcome: InviteOutcome::AlreadyPending,
                role: existing_invitation.role,
                state: existing_invitation.state,
                target_id: target.id,
            });
        }

        let invitation = Invitation::issue(account_id, target.id, role, actor_id, now);

        let stored = uow.accounts().create_invitation(&invitation).await?;
        uow.commit().await?;

        let outcome = if invitation.id == stored.id {
            InviteOutcome::Minted
        } else {
            InviteOutcome::AlreadyPending
        };

        Ok(Output {
            outcome,
            invitation_id: stored.id,
            account_id: stored.account,
            role: stored.role,
            state: stored.state,
            target_id: stored.invited_user,
        })
    }
}
