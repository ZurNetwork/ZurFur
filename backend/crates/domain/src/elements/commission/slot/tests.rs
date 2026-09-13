use chrono::Utc;

use super::*;
use crate::elements::did::Did;

// The title is trimmed on the way in, and a blank one is refused.
#[test]
fn a_slot_title_trims_and_rejects_blank() {
    assert_eq!(
        "  The knight  ".parse::<SlotTitle>().unwrap().as_str(),
        "The knight"
    );
    assert_eq!("".parse::<SlotTitle>(), Err(SlotTitleError::Empty));
    assert_eq!("   \t ".parse::<SlotTitle>(), Err(SlotTitleError::Empty));
}

// A new Slot's envelope: fresh id, address, acting user, title, notes.
#[test]
fn a_new_slot_carries_title_and_optional_notes() {
    let commission = CommissionId::new(uuid::Uuid::now_v7());
    let address = SurfaceAddress::new(
        crate::elements::commission::TabId::from(uuid::Uuid::now_v7()),
        "content".parse().unwrap(),
    );
    let owner = UserId::from(Did::from(format!("did:plc:{}", uuid::Uuid::now_v7())));
    let title = "The mage".parse::<SlotTitle>().unwrap();

    let slot = NewSlot::contributed_at(
        commission,
        address.clone(),
        title.clone(),
        Some("robes, not armor".to_string()),
        owner.clone(),
        Utc::now(),
    );

    assert_eq!(slot.commission_id, commission);
    assert_eq!(slot.address, address);
    assert_eq!(slot.title, title);
    assert_eq!(slot.notes.as_deref(), Some("robes, not armor"));
    assert_eq!(slot.created_by, owner);
}
