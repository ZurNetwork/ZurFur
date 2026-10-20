use strum::VariantArray;

use super::*;

// The direction-status tokens round-trip and never collide.
#[test]
fn direction_status_tokens_round_trip_and_never_collide() {
    let tokens: Vec<&'static str> = DirectionStatus::VARIANTS
        .iter()
        .map(<&'static str>::from)
        .collect();
    assert_eq!(
        tokens,
        [
            "waiting_for_input",
            "waiting_for_approval",
            "changes_requested"
        ]
    );

    for status in DirectionStatus::VARIANTS {
        let token = <&'static str>::from(status);
        assert_eq!(status.to_string(), token);
        assert_eq!(token.parse::<DirectionStatus>(), Ok(*status));
        assert_eq!(DirectionStatus::try_from(token), Ok(*status));
    }
}

// A token outside the vocabulary is refused, not guessed at.
#[test]
fn unknown_direction_status_tokens_do_not_parse() {
    assert_eq!(
        DirectionStatus::try_from("late"),
        Err(UnknownDirectionStatus),
        "deadline axis ≠ direction axis"
    );
    assert_eq!(DirectionStatus::try_from(""), Err(UnknownDirectionStatus));
    assert_eq!(
        DirectionStatus::try_from("Waiting for Input"),
        Err(UnknownDirectionStatus)
    );
}
