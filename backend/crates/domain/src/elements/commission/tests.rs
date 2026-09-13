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

// A fresh commission carries no deadline status, even born with a deadline.
#[test]
fn a_fresh_commission_has_no_deadline_status() {
    let c = Commission::create(
        "Ref".parse::<CommissionTitle>().unwrap(),
        crate::elements::user::UserId::from(crate::elements::did::Did::from(format!(
            "did:plc:{}",
            uuid::Uuid::now_v7()
        ))),
        chrono::Utc::now(),
        Some(chrono::Utc::now()),
    );
    assert_eq!(c.deadline_status, None);
}

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

// A fresh commission carries no direction status (the cleared state).
#[test]
fn a_fresh_commission_has_no_direction_status() {
    let c = Commission::create(
        "Ref".parse::<CommissionTitle>().unwrap(),
        crate::elements::user::UserId::from(crate::elements::did::Did::from(format!(
            "did:plc:{}",
            uuid::Uuid::now_v7()
        ))),
        chrono::Utc::now(),
        None,
    );
    assert_eq!(c.direction_status, None);
}
