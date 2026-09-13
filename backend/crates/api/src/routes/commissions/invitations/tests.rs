//! Pins the two response bodies' wire shapes.

use super::*;

#[test]
fn invite_to_seat_response_serializes_every_field_as_a_string() {
    let body = InviteToSeatResponse {
        commission: "commission-id".to_string(),
        id: "offer-id".to_string(),
        seat: "seat-id".to_string(),
        state: "pending",
        user: "did:plc:invitee".to_string(),
    };

    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        r#"{"commission":"commission-id","id":"offer-id","seat":"seat-id","state":"pending","user":"did:plc:invitee"}"#
    );
}

#[test]
fn revoke_seat_invitation_response_serializes_every_field_as_a_string() {
    let body = RevokeSeatInvitationResponse {
        commission: "commission-id".to_string(),
        seat: "seat-id".to_string(),
        user: "did:plc:invitee".to_string(),
    };

    assert_eq!(
        serde_json::to_string(&body).unwrap(),
        r#"{"commission":"commission-id","seat":"seat-id","user":"did:plc:invitee"}"#
    );
}
