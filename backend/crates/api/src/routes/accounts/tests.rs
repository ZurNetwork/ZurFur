//! Pins each response body's wire shape: every field a string.

use super::*;

#[test]
fn accept_invitation_response_serializes_every_field_as_a_string() {
    let body = AcceptInvitationResponse {
        account: "account-id".to_string(),
        role: "member".to_string(),
        user: "did:plc:invitee".to_string(),
    };
    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        r#"{"account":"account-id","role":"member","user":"did:plc:invitee"}"#
    );
}

#[test]
fn grant_role_response_serializes_every_field_as_a_string() {
    let body = GrantRoleResponse {
        account: "account-id".to_string(),
        role: "admin".to_string(),
        user: "did:plc:grantee".to_string(),
    };
    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        r#"{"account":"account-id","role":"admin","user":"did:plc:grantee"}"#
    );
}

#[test]
fn revoke_role_response_serializes_every_field_as_a_string() {
    let body = RevokeRoleResponse {
        account: "account-id".to_string(),
        user: "did:plc:target".to_string(),
    };
    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        r#"{"account":"account-id","user":"did:plc:target"}"#
    );
}

#[test]
fn invite_user_to_account_response_serializes_every_field_as_a_string() {
    let body = InviteUserToAccountResponse {
        account: "account-id".to_string(),
        id: "offer-id".to_string(),
        role: "member".to_string(),
        state: "pending".to_string(),
        user: "did:plc:invitee".to_string(),
    };
    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        r#"{"account":"account-id","id":"offer-id","role":"member","state":"pending","user":"did:plc:invitee"}"#
    );
}

#[test]
fn revoke_invitation_response_serializes_every_field_as_a_string() {
    let body = RevokeInvitationResponse {
        account: "account-id".to_string(),
        user: "did:plc:invitee".to_string(),
    };
    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        r#"{"account":"account-id","user":"did:plc:invitee"}"#
    );
}

#[test]
fn decline_invitation_response_serializes_every_field_as_a_string() {
    let body = DeclineInvitationResponse {
        account: "account-id".to_string(),
        user: "did:plc:invitee".to_string(),
    };
    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        r#"{"account":"account-id","user":"did:plc:invitee"}"#
    );
}

#[test]
fn transfer_ownership_response_serializes_every_field_as_a_string() {
    let body = TransferOwnershipResponse {
        account: "account-id".to_string(),
        owner: "did:plc:new-owner".to_string(),
        previous_owner: "did:plc:old-owner".to_string(),
    };
    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        r#"{"account":"account-id","owner":"did:plc:new-owner","previous_owner":"did:plc:old-owner"}"#
    );
}
