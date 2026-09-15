use super::*;
use crate::elements::commission::{CommissionId, ElementId};
use crate::elements::did::Did;
use crate::elements::invitation::{InvitationError, InvitationState};
use crate::elements::user::UserId;
use chrono::{Duration, Utc};

fn commission() -> CommissionId {
    CommissionId::new(uuid::Uuid::now_v7())
}

fn seat() -> ElementId {
    ElementId::from(uuid::Uuid::now_v7())
}

fn user() -> UserId {
    UserId::from(Did::from(format!("did:plc:{}", uuid::Uuid::now_v7())))
}

// Issuance captures all four facts and starts pending, stamped once.
#[test]
fn issue_builds_a_pending_invitation_recording_its_facts() {
    let (commission, seat, invited, inviter) = (commission(), seat(), user(), user());
    let now = Utc::now();

    let invitation = SeatInvitation::issue(commission, seat, invited.clone(), inviter.clone(), now);

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
    let mut invitation = SeatInvitation::issue(commission(), seat(), user(), user(), Utc::now());
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
