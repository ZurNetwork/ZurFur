//! The [`Invitation`]: a pending offer of account membership — the issuing half
//! of invite-then-accept. Joining someone else's account is consequential, so it
//! is consensual: an Owner or Admin issues the offer and the invited User must
//! accept before any membership exists.
//!
//! Authority to issue is the grant rule
//! ([`Role::can_grant`](crate::elements::role::Role::can_grant)); the inviter is
//! recorded because on acceptance they become the new member's Parent. There is
//! no expiry.

use std::ops::Deref;
use std::str::FromStr;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        account::AccountId,
        id::{IdError, parse_uuid},
        role::Role,
        user::UserId,
    },
};

/// The app-private key of an [`Invitation`] (UUIDv7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InvitationId(uuid::Uuid);

impl InvitationId {
    /// Wraps an already-minted UUIDv7; a fresh id is minted by
    /// [`Invitation::issue`].
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl Deref for InvitationId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for InvitationId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_uuid(s).map(Self)
    }
}

/// Where an invitation sits in its lifecycle. Pending from issuance until
/// accepted or revoked; both end states are terminal and there is no expiry.
/// Persisted as its lowercase [`as_str`](InvitationState::as_str) discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvitationState {
    /// Issued and awaiting the invited User's decision — the only state that
    /// may be revoked or accepted.
    Pending,
    /// The invited User accepted and the membership was minted. Terminal.
    Accepted,
    /// The issuer revoked the offer before it was accepted. Terminal.
    Revoked,
}

impl std::fmt::Display for InvitationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Accepted => write!(f, "accepted"),
            Self::Revoked => write!(f, "revoked"),
        }
    }
}
/// A stored invitation-state discriminant outside the three known states — a
/// schema-drift signal, not user input; carries the offending value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownInvitationState(pub String);

impl std::fmt::Display for UnknownInvitationState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "unknown invitation state {:?}", self.0)
    }
}

impl std::error::Error for UnknownInvitationState {}

impl TryFrom<String> for InvitationState {
    type Error = UnknownInvitationState;

    /// Parse a stored discriminant back into a state.
    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "pending" => Ok(InvitationState::Pending),
            "accepted" => Ok(InvitationState::Accepted),
            "revoked" => Ok(InvitationState::Revoked),
            _ => Err(UnknownInvitationState(value)),
        }
    }
}

impl InvitationState {
    /// The lowercase discriminant the store persists.
    pub fn as_str(&self) -> &'static str {
        match self {
            InvitationState::Pending => "pending",
            InvitationState::Accepted => "accepted",
            InvitationState::Revoked => "revoked",
        }
    }
}

/// Why an invitation lifecycle transition was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvitationError {
    /// The transition needs a pending invitation, but this one was already
    /// accepted or revoked.
    NotPending,
}

impl std::fmt::Display for InvitationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InvitationError::NotPending => {
                write!(f, "only a pending invitation can be revoked")
            }
        }
    }
}

impl std::error::Error for InvitationError {}

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
    /// let account = AccountId::new(Did::from("did:plc:acme".to_string()));
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
            id: InvitationId::new(uuid::Uuid::now_v7()),
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
    ///     AccountId::new(Did::from("did:plc:acme".to_string())),
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
