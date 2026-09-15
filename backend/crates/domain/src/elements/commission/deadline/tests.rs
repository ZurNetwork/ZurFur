use std::collections::BTreeSet;

use super::*;

// The deadline-status tokens round-trip and never collide.
#[test]
fn deadline_status_tokens_round_trip_and_never_collide() {
    let mut seen = BTreeSet::new();
    for status in DeadlineStatus::ALL {
        let token = status.as_str();
        assert!(seen.insert(token), "duplicate token {token:?}");
        assert_eq!(
            DeadlineStatus::try_from(token),
            Ok(*status),
            "token {token:?} must round-trip back to its value",
        );
    }
    assert_eq!(DeadlineStatus::ALL.len(), 2, "exactly the two values");
}

// A token outside the vocabulary is refused; the axes never bleed.
#[test]
fn unknown_deadline_status_tokens_do_not_parse() {
    assert_eq!(
        DeadlineStatus::try_from("waiting_for_input"),
        Err(DeadlineStatusError::InvalidValue),
        "direction axis ≠ deadline axis"
    );
    assert_eq!(
        DeadlineStatus::try_from(""),
        Err(DeadlineStatusError::InvalidValue)
    );
    assert_eq!(
        DeadlineStatus::try_from("Late"),
        Err(DeadlineStatusError::InvalidValue)
    );
}
