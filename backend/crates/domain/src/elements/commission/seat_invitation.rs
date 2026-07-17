//! The [`SeatInvitation`] — a pending offer of a commission **Seat**, the issuing
//! half of seat invite-then-accept (ZMVP-78; DESIGN/Commission, Referenceable/
//! Slot/Seat DD `28311564`).
//!
//! The Seat mirror of the account [`Invitation`](crate::elements::invitation::Invitation)
//! (ZMVP-32): a Seat is a *structural* participant position declared vacant
//! (ZMVP-76), and filling one is consensual — the commission owner *issues* a
//! `SeatInvitation`, and the invited User must *accept* before they occupy the
//! Seat (ZMVP-79). This element is the issued offer and its lifecycle.
//!
//! It **reuses the account invitation's state machine wholesale** — the same
//! [`InvitationState`] (`pending`/`accepted`/`revoked`, no expiry) and
//! [`InvitationError`] — rather than growing a parallel one: the seat lifecycle
//! is the identical pending→accepted|revoked shape, so there is nothing to
//! diverge. Scope split: this module (and ZMVP-78) only ever issues a
//! [`Pending`] offer or [`revoke`](SeatInvitation::revoke)s it to [`Revoked`];
//! the [`Accepted`] transition — and the seat occupancy it mints — lives in
//! ZMVP-79.
//!
//! [`Pending`]: InvitationState::Pending
//! [`Revoked`]: InvitationState::Revoked
//! [`Accepted`]: InvitationState::Accepted

use std::ops::Deref;

use crate::{
    datetime::DateTimeUtc,
    elements::{
        commission::{CommissionId, NodeId},
        invitation::{InvitationError, InvitationState},
        user::UserId,
    },
};

/// The app-private, stable handle for a [`SeatInvitation`].
///
/// A UUIDv7 wrapped for type safety, mirroring
/// [`InvitationId`](crate::elements::invitation::InvitationId) and
/// [`CommissionId`]: the app mints the key, the domain only names it. Deref
/// exposes the inner UUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SeatInvitationId(uuid::Uuid);

impl SeatInvitationId {
    /// Wraps an already-minted UUIDv7 — e.g. a row read back from the store. A
    /// *fresh* id is minted inside [`SeatInvitation::issue`], not here.
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

/// A pending (or once-pending) offer of a commission Seat: who is invited, to
/// which [`seat`](SeatInvitation::seat) of which [`commission`](SeatInvitation::commission),
/// by whom, and where it sits in its lifecycle.
///
/// Build one with [`SeatInvitation::issue`], which stamps it [`Pending`]; move it
/// to [`Revoked`] with [`revoke`](SeatInvitation::revoke). Like the account
/// [`Invitation`](crate::elements::invitation::Invitation), it is **not** `Clone`
/// — an entity with identity and a lifecycle, not a value to copy around; the
/// store rebuilds it from its parts on read.
///
/// The `inviter` is the commission owner (the route's owner-only authority gate
/// settles that before this is issued). Authority to *fill* the seat — and to
/// resolve which of several pending invitees wins the race — is ZMVP-79's, not
/// this element's; this is only the offer and its state.
///
/// References: [`SeatInvitation::issue`], [`SeatInvitation::revoke`],
/// [`CommissionWrites::create_seat_invitation`](crate::ports::CommissionWrites::create_seat_invitation),
/// DESIGN/Commission, DD `28311564`, ZMVP-78/ZMVP-79.
///
/// [`Pending`]: InvitationState::Pending
/// [`Revoked`]: InvitationState::Revoked
pub struct SeatInvitation {
    /// The app-private id, minted at issuance.
    pub id: SeatInvitationId,
    /// The commission whose Seat is offered.
    pub commission: CommissionId,
    /// The Seat being offered — its tree node id (the `commission_seat`
    /// satellite key). The invited User occupies it only by accepting (ZMVP-79).
    pub seat: NodeId,
    /// The User being invited. They fill the Seat only by accepting (ZMVP-79).
    pub invited_user: UserId,
    /// The commission owner who issued the offer (the route's owner-only gate
    /// settles authority before issuing).
    pub inviter: UserId,
    /// Where the offer sits in its lifecycle. [`Pending`](InvitationState::Pending)
    /// at issuance.
    pub state: InvitationState,
    /// When the invitation was issued; equals `updated_at` at issuance.
    pub created_at: DateTimeUtc,
    /// When the invitation last changed state (e.g. on revoke).
    pub updated_at: DateTimeUtc,
}

impl SeatInvitation {
    /// Issue a fresh, [`Pending`](InvitationState::Pending) seat invitation.
    ///
    /// Mints the id (`SeatInvitationId::new(Uuid::now_v7())`) and stamps
    /// `created_at == updated_at == now`. A pure builder, like
    /// [`Invitation::issue`](crate::elements::invitation::Invitation::issue): the
    /// authority to issue (the inviter being the commission owner, the seat being
    /// vacant) is the caller's check, settled before this is reached.
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, NodeId, SeatInvitation},
    ///     invitation::InvitationState,
    ///     user::UserId,
    /// };
    ///
    /// let commission = CommissionId::new(uuid::Uuid::now_v7());
    /// let seat = NodeId::new(uuid::Uuid::now_v7());
    /// let invited = UserId::new(uuid::Uuid::now_v7());
    /// let inviter = UserId::new(uuid::Uuid::now_v7());
    /// let invitation = SeatInvitation::issue(commission, seat, invited, inviter, Utc::now());
    ///
    /// assert_eq!(invitation.state, InvitationState::Pending); // issued pending
    /// assert_eq!(invitation.created_at, invitation.updated_at); // stamped once
    /// ```
    pub fn issue(
        commission: CommissionId,
        seat: NodeId,
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
    /// [`Revoked`](InvitationState::Revoked) and stamping `updated_at`.
    ///
    /// The pure encoding of "the owner may revoke a *pending* seat offer": only a
    /// [`Pending`](InvitationState::Pending) offer can be revoked — revoking one
    /// already accepted or revoked is [`InvitationError::NotPending`], leaving the
    /// state untouched. *Who* may revoke (the owner) is the caller's authority
    /// check; this guards only the state transition. Mirrors
    /// [`Invitation::revoke`](crate::elements::invitation::Invitation::revoke)
    /// exactly (the shared state machine).
    ///
    /// ```
    /// use chrono::Utc;
    /// use domain::elements::{
    ///     commission::{CommissionId, NodeId, SeatInvitation},
    ///     invitation::{InvitationError, InvitationState},
    ///     user::UserId,
    /// };
    ///
    /// let mut invitation = SeatInvitation::issue(
    ///     CommissionId::new(uuid::Uuid::now_v7()),
    ///     NodeId::new(uuid::Uuid::now_v7()),
    ///     UserId::new(uuid::Uuid::now_v7()),
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

    fn commission() -> CommissionId {
        CommissionId::new(uuid::Uuid::now_v7())
    }

    fn seat() -> NodeId {
        NodeId::new(uuid::Uuid::now_v7())
    }

    fn user() -> UserId {
        UserId::new(uuid::Uuid::now_v7())
    }

    // Issuance captures all four facts — the invited User, the commission, the
    // seat, and the inviter — and starts pending, stamped once.
    #[test]
    fn issue_builds_a_pending_invitation_recording_its_facts() {
        let (commission, seat, invited, inviter) = (commission(), seat(), user(), user());
        let now = Utc::now();

        let invitation = SeatInvitation::issue(commission, seat, invited, inviter, now);

        assert_eq!(invitation.commission, commission);
        assert_eq!(invitation.seat, seat);
        assert_eq!(invitation.invited_user, invited);
        assert_eq!(invitation.inviter, inviter);
        assert_eq!(invitation.state, InvitationState::Pending);
        assert_eq!(invitation.created_at, now);
        assert_eq!(invitation.updated_at, now);
    }

    // Revoking a pending invitation moves it to revoked and bumps updated_at,
    // leaving created_at (the issuance stamp) untouched.
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

    // The state guard: only a pending invitation revokes; a second revoke is
    // rejected and changes nothing (the shared InvitationState machine).
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
