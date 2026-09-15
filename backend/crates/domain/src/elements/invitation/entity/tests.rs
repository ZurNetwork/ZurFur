use super::*;
use crate::elements::did::Did;
use chrono::{Duration, Utc};

fn account() -> AccountId {
    AccountId::new(Did::from(format!("did:plc:{}", uuid::Uuid::now_v7())))
}

fn user() -> UserId {
    UserId::from(Did::from(format!("did:plc:{}", uuid::Uuid::now_v7())))
}

// Issuance captures all four facts and starts pending, stamped once.
#[test]
fn issue_builds_a_pending_invitation_recording_its_four_facts() {
    let (account, invited, inviter) = (account(), user(), user());
    let now = Utc::now();

    let invitation = Invitation::issue(
        account.clone(),
        invited.clone(),
        Role::Admin,
        inviter.clone(),
        now,
    );

    assert_eq!(invitation.account, account);
    assert_eq!(invitation.invited_user, invited);
    assert_eq!(invitation.role, Role::Admin);
    assert_eq!(invitation.inviter, inviter);
    assert_eq!(invitation.state, InvitationState::Pending);
    assert_eq!(invitation.created_at, now);
    assert_eq!(invitation.updated_at, now);
}

// Revoking bumps updated_at and leaves created_at untouched.
#[test]
fn revoke_moves_a_pending_invitation_to_revoked() {
    let issued = Utc::now();
    let mut invitation = Invitation::issue(account(), user(), Role::Member, user(), issued);
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
    let mut invitation = Invitation::issue(account(), user(), Role::Member, user(), Utc::now());
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

// Invite authority IS the grant rule, not a parallel one. The full
// actor->target matrix is exhausted in `role.rs`; this pins the binding.
#[test]
fn invite_authority_is_the_grant_rule() {
    assert!(
        Role::Owner.can_grant(&Role::Admin),
        "an Owner may invite an Admin"
    );
    assert!(
        !Role::Admin.can_grant(&Role::Admin),
        "an Admin may not invite a peer Admin"
    );
    for inviter in [Role::Manager, Role::Member] {
        assert!(
            !inviter.can_grant(&Role::Member),
            "{inviter:?} cannot invite anyone"
        );
    }
}
