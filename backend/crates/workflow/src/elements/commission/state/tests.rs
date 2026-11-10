use super::*;
use crate::elements::commission::test_commission;

// A state reads the stored flag, never a derived Late, and every other fact as stored.
#[test]
fn a_state_holds_the_stored_facts() {
    let deadline = "2026-03-01T12:00:00Z"
        .parse::<DateTimeUtc>()
        .expect("an RFC 3339 instant");
    let mut commission = test_commission::stored(LifecycleStep::Active, Some(deadline), None);
    commission.direction_status = Some(DirectionStatus::WaitingForApproval);

    let state = CommissionState::from(&commission);

    assert_eq!(state.lifecycle_step(), &LifecycleStep::Active);
    assert_eq!(state.deadline(), Some(deadline));
    assert_eq!(state.delayed_flag(), None);
    assert_eq!(
        state.direction_status(),
        Some(DirectionStatus::WaitingForApproval)
    );
    assert_eq!(state.maturity(), None);
    assert!(!state.is_disputed());
    let after = "2026-05-01T12:00:00Z"
        .parse::<DateTimeUtc>()
        .expect("an RFC 3339 instant");
    assert_eq!(state.deadline_status_at(after), Some(DeadlineStatus::Late));
    assert_eq!(
        commission.deadline_status_at(after),
        Some(DeadlineStatus::Late)
    );
}
