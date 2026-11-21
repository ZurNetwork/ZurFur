use strum::VariantArray;

use super::*;
use crate::elements::commission::test_commission;

/// An instant written as RFC 3339.
fn instant(text: &str) -> DateTimeUtc {
    text.parse::<DateTimeUtc>().expect("an RFC 3339 instant")
}

/// When the tests' disputes begin.
fn raised_at() -> DateTimeUtc {
    instant("2026-03-01T12:00:00Z")
}

/// Every step but Disputed, the steps a dispute can leave.
fn undisputed_steps() -> Vec<LifecycleStep> {
    LifecycleStep::VARIANTS
        .iter()
        .filter(|step| **step != LifecycleStep::Disputed)
        .cloned()
        .collect()
}

// A dispute is raised from any step, Draft included, carrying the step it left and when it
// began.
#[test]
fn a_dispute_is_raised_from_any_step() {
    for step in undisputed_steps() {
        let commission = test_commission::at(step.clone());
        let state = CommissionState::from(&commission);

        let raised = raise_dispute(&state, raised_at()).expect("a dispute is raised");

        assert_eq!(raised.before(), &step, "{step}");
        assert_eq!(raised.began(), raised_at(), "{step}");
    }
}

// A Disputed commission refuses a second raise.
#[test]
fn a_second_raise_is_refused() {
    let commission = test_commission::disputed(LifecycleStep::Active, raised_at(), None);
    let state = CommissionState::from(&commission);

    assert_eq!(
        raise_dispute(&state, raised_at()),
        Err(CommissionRefusal::CommissionDisputed)
    );
}

// A clear returns the commission to exactly the step the dispute left, Draft included.
#[test]
fn clearing_returns_the_step_it_left() {
    for step in undisputed_steps() {
        let commission = test_commission::disputed(step.clone(), raised_at(), None);
        let state = CommissionState::from(&commission);

        let cleared = clear_dispute(&state).expect("the dispute clears");

        assert_eq!(cleared.after(), &step, "{step}");
    }
}

// A clear is refused on every step but Disputed.
#[test]
fn a_clear_is_refused_when_not_disputed() {
    for step in undisputed_steps() {
        let commission = test_commission::at(step.clone());
        let state = CommissionState::from(&commission);

        assert_eq!(
            clear_dispute(&state),
            Err(CommissionRefusal::CommissionNotDisputed),
            "{step}"
        );
    }
}

// A raise then a clear, on the state each leaves, lands where it started.
#[test]
fn a_raise_then_a_clear_lands_where_it_started() {
    for step in undisputed_steps() {
        let before = CommissionState::from(&test_commission::at(step.clone()));
        let raised = raise_dispute(&before, raised_at()).expect("a dispute is raised");
        let during = test_commission::disputed(raised.before().clone(), raised.began(), None);

        let cleared = clear_dispute(&CommissionState::from(&during)).expect("the dispute clears");

        assert_eq!(cleared.after(), &step, "{step}");
    }
}
