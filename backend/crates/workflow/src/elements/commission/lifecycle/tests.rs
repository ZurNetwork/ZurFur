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

/// A stored commission at `step`, as a rule reads it.
fn state_at(step: LifecycleStep) -> CommissionState {
    let commission = crate::elements::commission::test_commission::at(step);
    CommissionState::from(&commission)
}

// No move enters Draft, from any step; a move into Disputed is refused too, and while
// Disputed nothing moves.
#[test]
fn no_move_enters_draft() {
    for from in LifecycleStep::VARIANTS {
        if matches!(from, LifecycleStep::Draft | LifecycleStep::Disputed) {
            continue;
        }
        let into_draft = move_lifecycle(&state_at(from.clone()), LifecycleStep::Draft);
        assert_eq!(
            into_draft,
            Err(CommissionRefusal::LifecycleMoveRefused),
            "{from} → draft"
        );
    }

    let into_disputed = move_lifecycle(&state_at(LifecycleStep::Active), LifecycleStep::Disputed);
    assert_eq!(into_disputed, Err(CommissionRefusal::LifecycleMoveRefused));

    for to in LifecycleStep::VARIANTS {
        let while_disputed = move_lifecycle(&state_at(LifecycleStep::Disputed), to.clone());
        assert_eq!(
            while_disputed,
            Err(CommissionRefusal::CommissionDisputed),
            "disputed → {to}"
        );
    }
}

// A move to the step already held is a no-op, Draft to Draft included.
#[test]
fn a_move_to_the_held_step_is_a_no_op() {
    for step in LifecycleStep::VARIANTS {
        if *step == LifecycleStep::Disputed {
            continue;
        }
        let held = move_lifecycle(&state_at(step.clone()), step.clone());
        assert_eq!(held, Ok(None), "{step} → {step}");
    }
}

// A move between working steps passes, carrying the step held and the step moved to.
#[test]
fn a_move_between_working_steps_carries_both_steps() {
    let moved = move_lifecycle(&state_at(LifecycleStep::Draft), LifecycleStep::Active)
        .expect("draft → active passes")
        .expect("a real move");

    assert_eq!(moved.before(), &LifecycleStep::Draft);
    assert_eq!(moved.after(), &LifecycleStep::Active);

    let reopened = move_lifecycle(&state_at(LifecycleStep::Completed), LifecycleStep::Active);
    assert!(matches!(reopened, Ok(Some(_))), "{reopened:?}");
}

// A missed deadline lapses on every working step: never on closed work, and never while
// Disputed, which isn't terminal.
#[test]
fn a_deadline_lapses_on_working_steps_only() {
    let not_lapsing: Vec<&'static str> = LifecycleStep::not_lapsing()
        .iter()
        .map(<&'static str>::from)
        .collect();

    assert_eq!(not_lapsing, ["completed", "cancelled", "disputed"]);
    for step in LifecycleStep::VARIANTS {
        let listed = LifecycleStep::not_lapsing().contains(step);
        assert_eq!(step.lapses(), !listed, "{step}");
    }
}

// Only a Draft commission may be hard-deleted: every other step, Disputed included, is
// refused.
#[test]
fn only_draft_allows_hard_delete() {
    let deletable: Vec<&'static str> = LifecycleStep::VARIANTS
        .iter()
        .filter(|step| step.allows_hard_delete())
        .map(<&'static str>::from)
        .collect();

    assert_eq!(deletable, ["draft"]);
}
