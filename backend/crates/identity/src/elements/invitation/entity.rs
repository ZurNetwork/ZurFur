use crate::{
    datetime::DateTimeUtc,
    elements::{account::AccountId, role::Role, user::UserId},
};

use super::{InvitationError, InvitationId, InvitationState};

/// A pending (or once-pending) offer of account membership: who is invited, to
/// which account, at what [`Role`], by whom, and where it sits in its lifecycle.
/// Build one with [`Invitation::issue`] and move it on with
/// [`revoke`](Invitation::revoke). Not `Clone` — an entity, not a value.
pub struct Invitation {
    /// The app-private id, minted at issuance.
    pub id: InvitationId,
    /// The account the invited User is offered membership of.
    pub account: AccountId,
    /// The User being invited; they become a member only by accepting.
    pub invited_user: UserId,
    /// The offered rank, strictly below the inviter's own.
    pub role: Role,
    /// The member who issued the offer — on acceptance, the new member's Parent.
    pub inviter: UserId,
    /// Where the offer sits in its lifecycle; `Pending` at issuance.
    pub state: InvitationState,
    /// When the invitation was issued; equals `updated_at` at issuance.
    pub created_at: DateTimeUtc,
    /// When the invitation last changed state (e.g. on revoke).
    pub updated_at: DateTimeUtc,
}

impl Invitation {
    /// Issue a fresh, [`Pending`](InvitationState::Pending) invitation: mints
    /// the id and stamps `created_at == updated_at == now`. A pure builder —
    /// authority to issue is the caller's
    /// [`can_grant`](crate::elements::role::Role::can_grant) check.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     account::AccountId, did::Did, invitation::{Invitation, InvitationState}, role::Role,
    ///     user::UserId,
    /// };
    ///
    /// let account = AccountId::from(Did::from("did:plc:acme".to_string()));
    /// let invited = UserId::from(Did::from("did:plc:alice".to_string()));
    /// let inviter = UserId::from(Did::from("did:plc:bob".to_string()));
    /// let invitation = Invitation::issue(account, invited, Role::Member, inviter, Utc::now());
    ///
    /// assert_eq!(invitation.state, InvitationState::Pending); // issued pending
    /// assert_eq!(invitation.created_at, invitation.updated_at); // stamped once
    /// ```
    pub fn issue(
        account: AccountId,
        invited_user: UserId,
        role: Role,
        inviter: UserId,
        now: DateTimeUtc,
    ) -> Invitation {
        Invitation {
            id: InvitationId::from(uuid::Uuid::now_v7()),
            account,
            invited_user,
            role,
            inviter,
            state: InvitationState::Pending,
            created_at: now,
            updated_at: now,
        }
    }

    /// Revoke a pending invitation, moving it to
    /// [`Revoked`](InvitationState::Revoked) and stamping `updated_at`. Only a
    /// pending offer revokes; otherwise [`InvitationError::NotPending`] and the
    /// state is untouched. *Who* may revoke is the caller's check.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     account::AccountId, did::Did, invitation::{Invitation, InvitationError, InvitationState},
    ///     role::Role, user::UserId,
    /// };
    ///
    /// let mut invitation = Invitation::issue(
    ///     AccountId::from(Did::from("did:plc:acme".to_string())),
    ///     UserId::from(Did::from("did:plc:alice".to_string())),
    ///     Role::Member,
    ///     UserId::from(Did::from("did:plc:bob".to_string())),
    ///     Utc::now(),
    /// );
    /// assert!(invitation.revoke(Utc::now()).is_ok());
    /// assert_eq!(invitation.state, InvitationState::Revoked);
    /// // A revoked invitation can't be revoked again.
    /// assert_eq!(invitation.revoke(Utc::now()), Err(InvitationError::NotPending));
    /// ```
    pub fn revoke(&mut self, now: DateTimeUtc) -> Result<(), InvitationError> {
        if self.state != InvitationState::Pending {
            return Err(InvitationError::NotPending);
        }
        self.state = InvitationState::Revoked;
        self.updated_at = now;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
