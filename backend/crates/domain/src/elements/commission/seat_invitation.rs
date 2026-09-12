//! The [`SeatInvitation`]: a pending offer of a commission Seat — the issuing
//! half of seat invite-then-accept. Filling a vacant Seat is consensual, so the
//! owner issues an offer and the invited User must accept.
//!
//! Reuses the account invitation's [`InvitationState`] machine wholesale. This
//! module only issues a pending offer or revokes it; the accepted transition and
//! the occupancy it mints live elsewhere.

use std::ops::Deref;
use std::str::FromStr;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, ElementId},
        id::{IdError, parse_uuid},
        invitation::{InvitationError, InvitationState},
        user::UserId,
    },
};

/// The app-private key of a [`SeatInvitation`] (UUIDv7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SeatInvitationId(uuid::Uuid);

impl SeatInvitationId {
    /// Wraps an already-minted UUIDv7; a fresh id is minted by
    /// [`SeatInvitation::issue`].
    pub fn new(id: uuid::Uuid) -> Self {
        Self(id)
    }
}

impl Deref for SeatInvitationId {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl FromStr for SeatInvitationId {
    type Err = IdError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_uuid(s).map(Self)
    }
}

/// A pending (or once-pending) offer of a commission Seat: who is invited, to
/// which seat of which commission, by whom, and where it sits in its lifecycle.
/// Build one with [`SeatInvitation::issue`] and move it on with
/// [`revoke`](SeatInvitation::revoke). Not `Clone` — an entity, not a value.
pub struct SeatInvitation {
    /// The app-private id, minted at issuance.
    pub id: SeatInvitationId,
    /// The commission whose Seat is offered.
    pub commission: CommissionId,
    /// The Seat being offered — its carrying element's id.
    pub seat: ElementId,
    /// The User being invited; they fill the Seat only by accepting.
    pub invited_user: UserId,
    /// The commission owner who issued the offer.
    pub inviter: UserId,
    /// Where the offer sits in its lifecycle; `Pending` at issuance.
    pub state: InvitationState,
    /// When the invitation was issued; equals `updated_at` at issuance.
    pub created_at: DateTimeUtc,
    /// When the invitation last changed state (e.g. on revoke).
    pub updated_at: DateTimeUtc,
}

impl SeatInvitation {
    /// Issue a fresh, [`Pending`](InvitationState::Pending) seat invitation:
    /// mints the id and stamps `created_at == updated_at == now`. A pure
    /// builder — authority to issue is the caller's check.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, ElementId, SeatInvitation},
    ///     did::Did,
    ///     invitation::InvitationState,
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::new(uuid::Uuid::now_v7());
    /// let seat = ElementId::new(uuid::Uuid::now_v7());
    /// let invited = UserId::new(Did::new("did:plc:alice".to_string()));
    /// let inviter = UserId::new(Did::new("did:plc:bob".to_string()));
    /// let invitation = SeatInvitation::issue(commission, seat, invited, inviter, Utc::now());
    ///
    /// assert_eq!(invitation.state, InvitationState::Pending); // issued pending
    /// assert_eq!(invitation.created_at, invitation.updated_at); // stamped once
    /// ```
    pub fn issue(
        commission: CommissionId,
        seat: ElementId,
        invited_user: UserId,
        inviter: UserId,
        now: DateTimeUtc,
    ) -> SeatInvitation {
        SeatInvitation {
            id: SeatInvitationId::new(uuid::Uuid::now_v7()),
            commission,
            seat,
            invited_user,
            inviter,
            state: InvitationState::Pending,
            created_at: now,
            updated_at: now,
        }
    }

    /// Revoke a pending seat invitation, moving it to
    /// [`Revoked`](InvitationState::Revoked) and stamping `updated_at`. Only a
    /// pending offer revokes; otherwise [`InvitationError::NotPending`] and the
    /// state is untouched. *Who* may revoke is the caller's check.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, ElementId, SeatInvitation},
    ///     did::Did,
    ///     invitation::{InvitationError, InvitationState},
    ///     user::UserId,
    /// };
    ///
    /// let mut invitation = SeatInvitation::issue(
    ///     CommissionId::new(uuid::Uuid::now_v7()),
    ///     ElementId::new(uuid::Uuid::now_v7()),
    ///     UserId::new(Did::new("did:plc:alice".to_string())),
    ///     UserId::new(Did::new("did:plc:bob".to_string())),
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
    use crate::elements::did::Did;
    use chrono::{Duration, Utc};

    fn commission() -> CommissionId {
        CommissionId::new(uuid::Uuid::now_v7())
    }

    fn seat() -> ElementId {
        ElementId::new(uuid::Uuid::now_v7())
    }

    fn user() -> UserId {
        UserId::new(Did::new(format!("did:plc:{}", uuid::Uuid::now_v7())))
    }

    // Issuance captures all four facts and starts pending, stamped once.
    #[test]
    fn issue_builds_a_pending_invitation_recording_its_facts() {
        let (commission, seat, invited, inviter) = (commission(), seat(), user(), user());
        let now = Utc::now();

        let invitation =
            SeatInvitation::issue(commission, seat, invited.clone(), inviter.clone(), now);

        assert_eq!(invitation.commission, commission);
        assert_eq!(invitation.seat, seat);
        assert_eq!(invitation.invited_user, invited);
        assert_eq!(invitation.inviter, inviter);
        assert_eq!(invitation.state, InvitationState::Pending);
        assert_eq!(invitation.created_at, now);
        assert_eq!(invitation.updated_at, now);
    }

    // Revoking bumps updated_at and leaves created_at untouched.
    #[test]
    fn revoke_moves_a_pending_invitation_to_revoked() {
        let issued = Utc::now();
        let mut invitation = SeatInvitation::issue(commission(), seat(), user(), user(), issued);
        let later = issued + Duration::seconds(30);

        assert_eq!(invitation.revoke(later), Ok(()));
        assert_eq!(invitation.state, InvitationState::Revoked);
        assert_eq!(invitation.updated_at, later, "revoke bumps updated_at");
        assert_eq!(
            invitation.created_at, issued,
            "created_at is the issuance stamp"
        );
    }

    // Only a pending invitation revokes; a second revoke changes nothing.
    #[test]
    fn revoking_a_non_pending_invitation_is_rejected() {
        let mut invitation =
            SeatInvitation::issue(commission(), seat(), user(), user(), Utc::now());
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
}
