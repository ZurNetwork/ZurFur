use std::collections::BTreeSet;

use super::*;

// The lifecycle tokens round-trip and the terminal set is exactly
// {completed, cancelled} — Disputed is not terminal.
#[test]
fn lifecycle_tokens_round_trip_and_terminal_is_exactly_closed_work() {
    let mut seen = BTreeSet::new();
    for step in LifecycleStep::ALL {
        let token = step.as_str();
        assert!(seen.insert(token), "duplicate token {token:?}");
        assert_eq!(
            LifecycleStep::try_from(token).map(|s| s.as_str()),
            Ok(token),
            "token {token:?} must round-trip back to its step",
        );
    }
    assert_eq!(LifecycleStep::ALL.len(), 6, "exactly the six states");

    let terminal: Vec<&str> = LifecycleStep::ALL
        .iter()
        .filter(|s| s.is_terminal())
        .map(|s| s.as_str())
        .collect();
    assert_eq!(terminal, vec!["completed", "cancelled"]);
}
