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

/// An instant written as RFC 3339.
fn instant(text: &str) -> DateTimeUtc {
    text.parse::<DateTimeUtc>().expect("an RFC 3339 instant")
}

/// The deadline the tests hold.
fn march() -> DateTimeUtc {
    instant("2026-03-01T12:00:00Z")
}

/// A later deadline.
fn april() -> DateTimeUtc {
    instant("2026-04-01T12:00:00Z")
}

/// Before the deadline.
fn february() -> DateTimeUtc {
    instant("2026-02-01T12:00:00Z")
}

/// After the deadline.
fn may() -> DateTimeUtc {
    instant("2026-05-01T12:00:00Z")
}

/// The state of a commission at `step` with `deadline` and the stored `flag`.
fn state(
    step: LifecycleStep,
    deadline: Option<DateTimeUtc>,
    flag: Option<DeadlineStatus>,
) -> CommissionState {
    let commission = crate::elements::commission::test_commission::stored(step, deadline, flag);
    CommissionState::from(&commission)
}

// While Disputed the deadline is frozen: a fresh set, an extension, a pull-in and a clear are
// all refused.
#[test]
fn a_disputed_commission_s_deadline_cannot_change() {
    let undated = state(LifecycleStep::Disputed, None, None);
    let dated = state(LifecycleStep::Disputed, Some(march()), None);

    assert_eq!(
        schedule_deadline(&undated, march()),
        Err(CommissionRefusal::CommissionDisputed)
    );
    assert_eq!(
        schedule_deadline(&dated, april()),
        Err(CommissionRefusal::CommissionDisputed)
    );
    assert_eq!(
        schedule_deadline(&dated, february()),
        Err(CommissionRefusal::CommissionDisputed)
    );
    assert_eq!(
        clear_deadline(&dated),
        Err(CommissionRefusal::CommissionDisputed)
    );
}

// A later deadline is an extension, an earlier one or a first one a set, the one held a no-op;
// a clear takes the held deadline away, and clearing none is a no-op.
#[test]
fn a_deadline_set_tells_an_extension_from_a_set() {
    let undated = state(LifecycleStep::Active, None, None);
    let dated = state(LifecycleStep::Active, Some(march()), None);

    let first = schedule_deadline(&undated, march())
        .expect("passes")
        .expect("a real set");
    assert_eq!(
        (first.before(), first.after(), first.extends()),
        (None, march(), false)
    );

    let later = schedule_deadline(&dated, april())
        .expect("passes")
        .expect("a real set");
    assert_eq!(
        (later.before(), later.after(), later.extends()),
        (Some(march()), april(), true)
    );

    let earlier = schedule_deadline(&dated, february())
        .expect("passes")
        .expect("a real set");
    assert!(!earlier.extends());

    assert_eq!(schedule_deadline(&dated, march()), Ok(None));

    let cleared = clear_deadline(&dated)
        .expect("passes")
        .expect("a real clear");
    assert_eq!(cleared.before(), march());
    assert_eq!(clear_deadline(&undated), Ok(None));

    let write = DeadlineChange::from(cleared);
    assert_eq!((write.before(), write.after()), (Some(march()), None));
}

// Late is derived at the time given: past the deadline on a working step it is Late whatever
// the stored flag, before it the stored flag, on a terminal step never, and with no deadline
// nothing.
#[test]
fn late_is_derived_at_the_given_time() {
    let unflagged = state(LifecycleStep::Active, Some(march()), None);
    let flagged = state(
        LifecycleStep::Active,
        Some(march()),
        Some(DeadlineStatus::Delayed),
    );
    let closed = state(LifecycleStep::Completed, Some(march()), None);
    let undated = state(LifecycleStep::Active, None, Some(DeadlineStatus::Delayed));

    assert_eq!(
        unflagged.deadline_status_at(may()),
        Some(DeadlineStatus::Late)
    );
    assert_eq!(
        flagged.deadline_status_at(may()),
        Some(DeadlineStatus::Late)
    );
    assert_eq!(unflagged.deadline_status_at(february()), None);
    assert_eq!(
        flagged.deadline_status_at(february()),
        Some(DeadlineStatus::Delayed)
    );
    assert_eq!(closed.deadline_status_at(may()), None);
    assert_eq!(undated.deadline_status_at(may()), None);
    assert_eq!(unflagged.delayed_flag(), None, "Late is never stored");
}

// While Late (derived, the stored flag empty) the Delayed flag can be neither raised nor
// taken down.
#[test]
fn delayed_is_refused_while_late() {
    let late = state(LifecycleStep::Active, Some(march()), None);
    let late_and_flagged = state(
        LifecycleStep::Active,
        Some(march()),
        Some(DeadlineStatus::Delayed),
    );

    assert_eq!(late.delayed_flag(), None);
    assert_eq!(
        set_delayed(&late, may()),
        Err(CommissionRefusal::CommissionLate)
    );
    assert_eq!(
        clear_delayed(&late_and_flagged, may()),
        Err(CommissionRefusal::CommissionLate)
    );
}

// The Delayed flag needs a deadline to stand against; taking it down doesn't.
#[test]
fn delayed_needs_a_deadline() {
    let undated = state(LifecycleStep::Active, None, None);
    let lingering = state(LifecycleStep::Active, None, Some(DeadlineStatus::Delayed));

    assert_eq!(
        set_delayed(&undated, may()),
        Err(CommissionRefusal::NoDeadline)
    );

    let lowered = clear_delayed(&lingering, may())
        .expect("passes")
        .expect("a real clear");
    assert_eq!((lowered.before(), lowered.deadline()), (None, None));
}

// A flag left after its deadline was cleared is seen as nothing, yet the row still holds it:
// the clear carries the stored flag apart from the status it saw, and its write compares
// against the stored one.
#[test]
fn a_dormant_flag_s_clear_compares_against_the_stored_flag() {
    let lingering = state(LifecycleStep::Active, None, Some(DeadlineStatus::Delayed));

    let lowered = clear_delayed(&lingering, may())
        .expect("passes")
        .expect("a real clear");
    let change = DelayedChange::from(lowered.clone());

    assert_eq!(lowered.before(), None);
    assert_eq!(lowered.held(), Some(DeadlineStatus::Delayed));
    assert_eq!(
        (change.before(), change.held(), change.after()),
        (None, Some(DeadlineStatus::Delayed), None)
    );
}

// A set carries the flag the row held, nothing, beside the status it saw.
#[test]
fn a_delayed_set_carries_the_stored_flag() {
    let dated = state(LifecycleStep::Active, Some(march()), None);

    let raised = set_delayed(&dated, february())
        .expect("passes")
        .expect("a real set");
    let change = DelayedChange::from(raised.clone());

    assert_eq!(raised.held(), None);
    assert_eq!(
        (change.held(), change.after()),
        (None, Some(DeadlineStatus::Delayed))
    );
}

// While Disputed the Delayed flag may still be raised and taken down; raising it twice or
// lowering it when none is stored is a no-op.
#[test]
fn delayed_while_disputed_answers_as_q7_rules() {
    let disputed = state(LifecycleStep::Disputed, Some(march()), None);
    let disputed_flagged = state(
        LifecycleStep::Disputed,
        Some(march()),
        Some(DeadlineStatus::Delayed),
    );

    let raised = set_delayed(&disputed, february())
        .expect("allowed while disputed")
        .expect("a real set");
    assert_eq!(
        (raised.before(), raised.after(), raised.deadline()),
        (None, DeadlineStatus::Delayed, Some(march()))
    );
    let lowered = clear_delayed(&disputed_flagged, february())
        .expect("allowed while disputed")
        .expect("a real clear");
    assert_eq!(lowered.before(), Some(DeadlineStatus::Delayed));

    assert_eq!(set_delayed(&disputed_flagged, february()), Ok(None));
    assert_eq!(clear_delayed(&disputed, february()), Ok(None));
}

/// The Late facts of a commission at `step` with the deadline in March, owed or not.
fn late_facts(step: LifecycleStep, owed: bool) -> LateFacts {
    LateFacts {
        state: state(step, Some(march()), Some(DeadlineStatus::Delayed)),
        owed,
    }
}

// A Late entry is owed once the deadline is strictly past on a working step, carrying the
// deadline and the stored flag it supersedes.
#[test]
fn a_late_entry_is_owed_once_the_deadline_is_past() {
    let owed = late_facts(LifecycleStep::Active, true);

    let mark = owes_late(&owed, may()).expect("owed");
    assert_eq!(
        (mark.deadline(), mark.before()),
        (march(), Some(DeadlineStatus::Delayed))
    );
    assert_eq!(
        owes_late(&owed, february()),
        Err(CommissionRefusal::NothingOwed)
    );
    assert_eq!(
        owes_late(&owed, march()),
        Err(CommissionRefusal::NothingOwed)
    );
}

// No Late entry is owed on a terminal step, while Disputed, once recorded, or with no deadline.
#[test]
fn no_late_entry_is_owed_when_closed_disputed_recorded_or_undated() {
    for step in [LifecycleStep::Completed, LifecycleStep::Cancelled] {
        let closed = late_facts(step.clone(), true);
        assert_eq!(
            owes_late(&closed, may()),
            Err(CommissionRefusal::NothingOwed),
            "{step}"
        );
    }
    let disputed = late_facts(LifecycleStep::Disputed, true);
    assert_eq!(
        owes_late(&disputed, may()),
        Err(CommissionRefusal::NothingOwed)
    );
    let recorded = late_facts(LifecycleStep::Active, false);
    assert_eq!(
        owes_late(&recorded, may()),
        Err(CommissionRefusal::NothingOwed)
    );
    let undated = LateFacts {
        state: state(LifecycleStep::Active, None, None),
        owed: true,
    };
    assert_eq!(
        owes_late(&undated, may()),
        Err(CommissionRefusal::NothingOwed)
    );
}
