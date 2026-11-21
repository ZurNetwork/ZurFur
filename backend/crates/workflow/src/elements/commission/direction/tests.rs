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

/// The state of an Active commission holding `direction`.
fn holding(direction: Option<DirectionStatus>) -> CommissionState {
    let mut commission = crate::elements::commission::test_commission::at(
        crate::elements::commission::LifecycleStep::Active,
    );
    commission.direction_status = direction;
    CommissionState::from(&commission)
}

// A set replaces whatever is held and the held value is a no-op; a clear takes the held value
// away and clearing none is a no-op. Each gives the store's write its from and to.
#[test]
fn direction_set_and_clear_are_no_ops_on_the_held_value() {
    let unset = holding(None);
    let waiting = holding(Some(DirectionStatus::WaitingForInput));

    let first = set_direction(&unset, DirectionStatus::WaitingForInput).expect("a real set");
    assert_eq!(
        (first.before(), first.after()),
        (None, DirectionStatus::WaitingForInput)
    );
    assert_eq!(
        set_direction(&waiting, DirectionStatus::WaitingForInput),
        None
    );

    let replaced = set_direction(&waiting, DirectionStatus::ChangesRequested).expect("a real set");
    let write = DirectionChange::from(replaced);
    assert_eq!(
        (write.before(), write.after()),
        (
            Some(DirectionStatus::WaitingForInput),
            Some(DirectionStatus::ChangesRequested)
        )
    );

    let cleared = clear_direction(&waiting).expect("a real clear");
    let write = DirectionChange::from(cleared);
    assert_eq!(
        (write.before(), write.after()),
        (Some(DirectionStatus::WaitingForInput), None)
    );
    assert_eq!(clear_direction(&unset), None);
}
