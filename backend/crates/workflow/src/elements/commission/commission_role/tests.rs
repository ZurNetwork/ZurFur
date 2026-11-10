use strum::VariantArray;

use super::*;

// The roles run Participant then Owner, and Participant ranks below Owner, so a
// variant added later can't outrank Owner by accident.
#[test]
fn the_order_is_participant_then_owner() {
    assert_eq!(
        CommissionRole::VARIANTS,
        [CommissionRole::Participant, CommissionRole::Owner]
    );
    assert!(CommissionRole::Participant < CommissionRole::Owner);
    let highest = CommissionRole::VARIANTS.iter().max();
    assert_eq!(highest, Some(&CommissionRole::Owner));
}

// Owning wins over a participant row, which wins over nothing.
#[test]
fn the_held_role_is_owner_then_participant_then_none() {
    assert_eq!(
        CommissionRole::held(true, false),
        Some(CommissionRole::Owner)
    );
    assert_eq!(
        CommissionRole::held(true, true),
        Some(CommissionRole::Owner)
    );
    assert_eq!(
        CommissionRole::held(false, true),
        Some(CommissionRole::Participant)
    );
    assert_eq!(CommissionRole::held(false, false), None);
}
