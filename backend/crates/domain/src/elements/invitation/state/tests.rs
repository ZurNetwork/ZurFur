use super::*;

// The persisted discriminant round-trips; an unknown one is a typed error.
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
