//! The [`Invitation`] — a pending offer of account membership, the issuing half
//! of invite-then-accept (ZMVP-32; DESIGN/1DD decision 11, DESIGN/Roles).
//!
//! Seating a member directly is a *grant* ([`crate::elements::role`], ZMVP-15).
//! But joining someone else's account is consequential — shared brand, wallet,
//! plugin entitlements, authority — so it is consensual: an Owner or Admin
//! *issues* an `Invitation`, and the invited User must *accept* before any
//! membership exists. This element is the issued offer and its lifecycle; the
//! authority to issue reuses the grant rule ([`Role::can_grant`] — the offered
//! role sits strictly below the inviter's own rank), and the inviter is recorded
//! because on acceptance they become the new member's Parent (DESIGN/Roles rule
//! 4a).
//!
//! Scope split: this module (and ZMVP-32) only ever issues a [`Pending`] offer or
//! [`revoke`](Invitation::revoke)s it to [`Revoked`]. The [`Accepted`] transition
//! — and the membership it mints — lives in ZMVP-20. There is no expiry: an
//! invitation stays valid until accepted or revoked.
//!
//! [`Pending`]: InvitationState::Pending
//! [`Revoked`]: InvitationState::Revoked
//! [`Accepted`]: InvitationState::Accepted
//! [`Role::can_grant`]: crate::elements::role::Role::can_grant

use std::ops::Deref;

use crate::{
    datetime::DateTimeUtc,
    elements::{account::AccountId, role::Role, user::UserId},
};

/// The app-private, stable handle for an [`Invitation`].
///
/// A UUIDv7 wrapped for type safety, mirroring [`AccountId`] and
/// [`crate::elements::user::UserId`]: the app mints the key, the domain only
/// names it. Deref exposes the inner UUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InvitationId(uuid::Uuid);

impl InvitationId {
    /// Wraps an already-minted UUIDv7 — e.g. a row read back from the store.
    /// A *fresh* id is minted inside [`Invitation::issue`], not here.
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

/// Where an invitation sits in its lifecycle.
///
/// An offer is [`Pending`](InvitationState::Pending) from issuance until it is
/// either [`Accepted`](InvitationState::Accepted) (ZMVP-20, which mints the
/// membership) or [`Revoked`](InvitationState::Revoked) by the issuer (ZMVP-32).
/// Both end states are terminal — a revoked invitation can never be accepted, and
/// an accepted one is spent. There is no expiry.
///
/// A fieldless enum, so it is `Copy`; persisted as its lowercase
/// [`as_str`](InvitationState::as_str) discriminant, the inverse of
/// [`TryFrom<String>`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvitationState {
    /// Issued and awaiting the invited User's decision. The only state ZMVP-32
    /// writes on creation, and the only state that may be revoked or accepted.
    Pending,
    /// The invited User accepted; the membership has been minted (ZMVP-20).
    /// Terminal.
    Accepted,
    /// The issuer revoked the offer before it was accepted (ZMVP-32). Terminal —
    /// a revoked invitation can no longer be accepted.
    Revoked,
}

/// A stored invitation-state discriminant that isn't one of the three known
/// states — a schema/data drift signal, not user input. Mirrors
/// [`crate::elements::role::UnknownRole`]; carries the offending value.
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

    /// Parse a stored discriminant (`pending` | `accepted` | `revoked`) back into
    /// a state. The inverse of [`as_str`](InvitationState::as_str).
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
    /// The lowercase discriminant (`pending` | `accepted` | `revoked`) — the value
    /// the store persists, and the inverse of [`TryFrom<String>`].
    pub fn as_str(&self) -> &'static str {
        match self {
            InvitationState::Pending => "pending",
            InvitationState::Accepted => "accepted",
            InvitationState::Revoked => "revoked",
        }
    }
}

/// Why an invitation lifecycle transition was refused.
///
/// Today only [`revoke`](Invitation::revoke) can fail, and only one way — the
/// offer wasn't pending. An enum (not a unit error) so the accept path (ZMVP-20)
/// can extend it without a breaking change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvitationError {
    /// The transition needs a [`Pending`](InvitationState::Pending) invitation,
    /// but this one was already accepted or revoked. Both end states are terminal.
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
/// which [`AccountId`], at what [`Role`], by whom, and where it sits in its
/// lifecycle.
///
/// Build one with [`Invitation::issue`], which stamps it [`Pending`]; move it to
/// [`Revoked`] with [`revoke`](Invitation::revoke). Like [`Account`], it is not
/// `Clone` — an entity with identity and a lifecycle, not a value to copy around;
/// the store rebuilds it from its parts on read.
///
/// The `inviter` is kept deliberately: per DESIGN/Roles rule 4a, on acceptance
/// (ZMVP-20) the inviter becomes the new member's Parent in the role tree. The
/// `role` is the *offered* rank; the rule that it sits strictly below the
/// inviter's own rank is the grant rule ([`Role::can_grant`]), checked by the
/// caller before issuing — the same authority seam grants use.
///
/// References: [`Invitation::issue`], [`Invitation::revoke`],
/// [`crate::ports::AccountRepo::create_invitation`], DESIGN/1DD decision 11,
/// DESIGN/Roles, ZMVP-32/ZMVP-20.
///
/// [`Pending`]: InvitationState::Pending
/// [`Revoked`]: InvitationState::Revoked
/// [`Account`]: crate::elements::account::Account
/// [`Role::can_grant`]: crate::elements::role::Role::can_grant
pub struct Invitation {
    /// The app-private id, minted at issuance.
    pub id: InvitationId,
    /// The account the invited User is offered membership of.
    pub account: AccountId,
    /// The User being invited. They become a member only by accepting (ZMVP-20).
    pub invited_user: UserId,
    /// The offered rank. Sits strictly below the inviter's own (the grant rule,
    /// checked before issuing). Carries the parent slot like any [`Role`], `None`
    /// here until the role tree lands.
    pub role: Role,
    /// The member who issued the offer. Recorded because on acceptance they become
    /// the new member's Parent (DESIGN/Roles rule 4a).
    pub inviter: UserId,
    /// Where the offer sits in its lifecycle. [`Pending`](InvitationState::Pending)
    /// at issuance.
    pub state: InvitationState,
    /// When the invitation was issued; equals `updated_at` at issuance.
    pub created_at: DateTimeUtc,
    /// When the invitation last changed state (e.g. on revoke).
    pub updated_at: DateTimeUtc,
}

impl Invitation {
    /// Issue a fresh, [`Pending`](InvitationState::Pending) invitation.
    ///
    /// Mints the id (`InvitationId::new(Uuid::now_v7())`) and stamps `created_at
    /// == updated_at == now`. A pure builder, like [`Account::open`]: the
    /// authority to issue (the offered `role` strictly below the inviter's rank,
    /// and the inviter being Owner/Admin) is the caller's check via
    /// [`Role::can_grant`], settled before this is reached — exactly as a grant
    /// settles authority before [`grant_role`](crate::ports::AccountRepo::grant_role).
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     account::AccountId, invitation::{Invitation, InvitationState}, role::Role, user::UserId,
    /// };
    ///
    /// let account = AccountId::new(uuid::Uuid::now_v7());
    /// let invited = UserId::new(uuid::Uuid::now_v7());
    /// let inviter = UserId::new(uuid::Uuid::now_v7());
    /// let invitation = Invitation::issue(account, invited, Role::Member(None), inviter, Utc::now());
    ///
    /// assert_eq!(invitation.state, InvitationState::Pending); // issued pending
    /// assert_eq!(invitation.created_at, invitation.updated_at); // stamped once
    /// ```
    ///
    /// [`Account::open`]: crate::elements::account::Account::open
    /// [`Role::can_grant`]: crate::elements::role::Role::can_grant
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
    /// [`Revoked`](InvitationState::Revoked) and stamping `updated_at`.
    ///
    /// The pure encoding of "the issuing member may revoke a *pending*
    /// invitation": only a [`Pending`](InvitationState::Pending) offer can be
    /// revoked — revoking one already accepted or revoked is
    /// [`InvitationError::NotPending`], leaving the state untouched. *Who* may
    /// revoke (the inviter) is the caller's authority check, like the grant seam;
    /// this guards only the state transition.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     account::AccountId, invitation::{Invitation, InvitationError, InvitationState},
    ///     role::Role, user::UserId,
    /// };
    ///
    /// let mut invitation = Invitation::issue(
    ///     AccountId::new(uuid::Uuid::now_v7()),
    ///     UserId::new(uuid::Uuid::now_v7()),
    ///     Role::Member(None),
    ///     UserId::new(uuid::Uuid::now_v7()),
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
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

    fn account() -> AccountId {
        AccountId::new(uuid::Uuid::now_v7())
    }

    fn user() -> UserId {
        UserId::new(uuid::Uuid::now_v7())
    }

    // AC3 — "a pending invitation records the invited User, the Account, the
    // offered role, and the inviter." Issuance captures all four and starts pending,
    // stamped once.
    #[test]
    fn issue_builds_a_pending_invitation_recording_its_four_facts() {
        let (account, invited, inviter) = (account(), user(), user());
        let now = Utc::now();

        let invitation = Invitation::issue(account, invited, Role::Admin(None), inviter, now);

        assert_eq!(invitation.account, account);
        assert_eq!(invitation.invited_user, invited);
        assert_eq!(invitation.role, Role::Admin(None));
        assert_eq!(invitation.inviter, inviter);
        assert_eq!(invitation.state, InvitationState::Pending);
        assert_eq!(invitation.created_at, now);
        assert_eq!(invitation.updated_at, now);
    }

    // AC4 — revoking a pending invitation moves it to revoked and bumps updated_at,
    // leaving created_at (the issuance stamp) untouched.
    #[test]
    fn revoke_moves_a_pending_invitation_to_revoked() {
        let issued = Utc::now();
        let mut invitation =
            Invitation::issue(account(), user(), Role::Member(None), user(), issued);
        let later = issued + Duration::seconds(30);

        assert_eq!(invitation.revoke(later), Ok(()));
        assert_eq!(invitation.state, InvitationState::Revoked);
        assert_eq!(invitation.updated_at, later, "revoke bumps updated_at");
        assert_eq!(
            invitation.created_at, issued,
            "created_at is the issuance stamp"
        );
    }

    // AC4 — "a revoked invitation can no longer be accepted." The state guard: only
    // a pending invitation revokes; a second revoke is rejected and changes nothing.
    #[test]
    fn revoking_a_non_pending_invitation_is_rejected() {
        let mut invitation =
            Invitation::issue(account(), user(), Role::Member(None), user(), Utc::now());
        invitation
            .revoke(Utc::now())
            .expect("first revoke succeeds");
        let stamp = invitation.updated_at;

        assert_eq!(
            invitation.revoke(Utc::now()),
            Err(InvitationError::NotPending),
            "a revoked invitation cannot be revoked again"
        );
        assert_eq!(
            invitation.state,
            InvitationState::Revoked,
            "state is unchanged"
        );
        assert_eq!(
            invitation.updated_at, stamp,
            "a rejected revoke bumps nothing"
        );
    }

    // The persisted discriminant round-trips, and an unknown one is a typed error
    // (schema drift), mirroring Role. Covers every variant.
    #[test]
    fn state_round_trips_through_its_discriminant() {
        for state in [
            InvitationState::Pending,
            InvitationState::Accepted,
            InvitationState::Revoked,
        ] {
            let parsed = InvitationState::try_from(state.as_str().to_string());
            assert_eq!(parsed, Ok(state), "{state:?} round-trips");
        }
        assert_eq!(
            InvitationState::try_from("expired".to_string()),
            Err(UnknownInvitationState("expired".to_string())),
            "an unknown discriminant is a typed error, not a panic"
        );
    }

    // AC2 — "the offered role sits strictly below the inviter's own rank (Manager
    // and Member cannot invite)." Invite authority is *the grant rule*, not a new
    // one: this pins the reuse so issuance and granting can never drift. The full
    // actor->target matrix is exhausted in `role.rs`; here we assert the binding.
    #[test]
    fn invite_authority_is_the_grant_rule() {
        assert!(
            Role::Owner(None).can_grant(&Role::Admin(None)),
            "an Owner may invite an Admin"
        );
        assert!(
            !Role::Admin(None).can_grant(&Role::Admin(None)),
            "an Admin may not invite a peer Admin"
        );
        for inviter in [Role::Manager(None), Role::Member(None)] {
            assert!(
                !inviter.can_grant(&Role::Member(None)),
                "{inviter:?} cannot invite anyone"
            );
        }
    }
}
