use strum::VariantArray;

use super::*;

// The lifecycle tokens round-trip and the terminal set is exactly
// {completed, cancelled} — Disputed is not terminal.
#[test]
fn lifecycle_tokens_round_trip_and_terminal_is_exactly_closed_work() {
    let tokens: Vec<&'static str> = LifecycleStep::VARIANTS
        .iter()
        .map(<&'static str>::from)
        .collect();
    assert_eq!(
        tokens,
        [
            "draft",
            "batched",
            "active",
            "completed",
            "cancelled",
            "disputed"
        ]
    );

    for step in LifecycleStep::VARIANTS {
        let token = <&'static str>::from(step);
        assert_eq!(step.to_string(), token);
        assert_eq!(token.parse::<LifecycleStep>(), Ok(step.clone()));
        assert_eq!(LifecycleStep::try_from(token), Ok(step.clone()));
    }

    let terminal: Vec<&str> = LifecycleStep::VARIANTS
        .iter()
        .filter(|s| s.is_terminal())
        .map(<&'static str>::from)
        .collect();
    assert_eq!(terminal, vec!["completed", "cancelled"]);
}

// A token outside the vocabulary is refused, not guessed at.
#[test]
fn unknown_lifecycle_tokens_do_not_parse() {
    assert_eq!(LifecycleStep::try_from(""), Err(UnknownLifecycleStep));
    assert_eq!(LifecycleStep::try_from("DRAFT"), Err(UnknownLifecycleStep));
    assert_eq!(
        LifecycleStep::try_from("archived"),
        Err(UnknownLifecycleStep)
    );
}
