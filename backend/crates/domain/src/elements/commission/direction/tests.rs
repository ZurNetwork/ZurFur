use std::collections::BTreeSet;

use super::*;

// The direction-status tokens round-trip and never collide.
#[test]
fn direction_status_tokens_round_trip_and_never_collide() {
    let mut seen = BTreeSet::new();
    for status in DirectionStatus::ALL {
        let token = status.as_str();
        assert!(seen.insert(token), "duplicate token {token:?}");
        assert_eq!(
            DirectionStatus::try_from(token),
            Ok(*status),
            "token {token:?} must round-trip back to its value",
        );
    }
    assert_eq!(DirectionStatus::ALL.len(), 3, "exactly the three values");
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
