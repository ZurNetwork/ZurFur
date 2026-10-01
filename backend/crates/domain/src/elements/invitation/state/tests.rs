use super::*;
use strum::VariantArray;

// The persisted discriminant round-trips; an unknown one is a typed error.
#[test]
fn state_round_trips_through_its_discriminant() {
    let tokens: Vec<&'static str> = InvitationState::VARIANTS
        .iter()
        .map(<&'static str>::from)
        .collect();
    assert_eq!(tokens, ["pending", "accepted", "revoked"]);

    for state in InvitationState::VARIANTS {
        let token = <&'static str>::from(state);
        assert_eq!(state.to_string(), token);
        assert_eq!(token.parse::<InvitationState>(), Ok(*state));
        assert_eq!(InvitationState::try_from(token), Ok(*state));
    }

    assert_eq!(
        "expired".parse::<InvitationState>(),
        Err(UnknownInvitationState("expired".to_string())),
        "an unknown discriminant is a typed error, not a panic"
    );
    assert_eq!(
        "".parse::<InvitationState>(),
        Err(UnknownInvitationState(String::new())),
        "an empty token is a typed error, not a panic"
    );
}

// InvitationState is one of the two enums whose old parser lowercased its
// input, so it stays case-insensitive and keeps the offending token verbatim
// in its error.
#[test]
fn state_parses_case_insensitively_and_keeps_the_offending_input() {
    assert_eq!(
        "PENDING".parse::<InvitationState>(),
        Ok(InvitationState::Pending)
    );
    assert_eq!(
        "Accepted".parse::<InvitationState>(),
        Ok(InvitationState::Accepted)
    );
    assert_eq!(
        "Expired".parse::<InvitationState>(),
        Err(UnknownInvitationState("Expired".to_string())),
        "the offending token keeps its original case"
    );
}
