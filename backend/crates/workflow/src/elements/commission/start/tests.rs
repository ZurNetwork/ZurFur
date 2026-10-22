use super::*;

// A new commission starts Draft, with no direction or deadline status, no linked channel,
// and no archive time.
#[test]
fn a_new_commission_starts_draft_unstatused_unlinked_and_live() {
    let draft_unstatused_unlinked_live = CommissionStart {
        lifecycle_step: LifecycleStep::Draft,
        direction_status: None,
        deadline_status: None,
        linked_channel: None,
        archived_at: None,
    };
    assert_eq!(CommissionStart::default(), draft_unstatused_unlinked_live);
}
