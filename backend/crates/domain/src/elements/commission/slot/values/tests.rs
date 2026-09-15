use super::super::SlotTitleError;
use super::*;

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
