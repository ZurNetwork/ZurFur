use strum::VariantArray;

use super::*;

// The deadline-status tokens round-trip and never collide.
#[test]
fn deadline_status_tokens_round_trip_and_never_collide() {
    let tokens: Vec<&'static str> = DeadlineStatus::VARIANTS
        .iter()
        .map(<&'static str>::from)
        .collect();
    assert_eq!(tokens, ["delayed", "late"]);

    for status in DeadlineStatus::VARIANTS {
        let token = <&'static str>::from(status);
        assert_eq!(status.to_string(), token);
        assert_eq!(token.parse::<DeadlineStatus>(), Ok(*status));
        assert_eq!(DeadlineStatus::try_from(token), Ok(*status));
    }
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
