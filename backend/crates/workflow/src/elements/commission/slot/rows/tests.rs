use chrono::Utc;

use super::*;
use crate::elements::commission::TabId;
use kernel::elements::did::Did;

/// A new Slot at a fresh tab's `content` surface, with `notes`.
fn new_slot(notes: Option<&str>) -> NewSlot {
    let address = SurfaceAddress::new(
        TabId::from(uuid::Uuid::now_v7()),
        "content".parse().unwrap(),
    );
    NewSlot {
        address,
        notes: notes.map(str::to_owned),
    }
}

// A minted declaration takes a fresh v7 element id each time and keeps what was chosen.
#[test]
fn a_minted_declaration_takes_a_fresh_element_id() {
    let slot = new_slot(Some("robes, not armor"));

    let first = SlotDeclaration::minted(slot.clone());
    let second = SlotDeclaration::minted(slot.clone());

    assert_ne!(first.element, second.element);
    assert_eq!(first.slot, slot);
    let minted = uuid::Uuid::from(first.element);
    assert_eq!(minted.get_version_num(), 7);
}

// No Debug of a new Slot, its declaration or its birth prints the notes or the declarer's DID.
#[test]
fn slot_debugs_print_no_notes_and_no_did() {
    let canary = "Canary notes 9c1e";
    let did = "did:plc:canarydeclarer";
    let declaration = SlotDeclaration::minted(new_slot(Some(canary)));
    let birth = SlotBirth {
        element: declaration.element,
        node: NodeId::mint(),
        created_by: UserId::from(Did::from(did.to_owned())),
        created_at: Utc::now(),
    };

    let printed = [
        format!("{:?}", declaration.slot),
        format!("{declaration:?}"),
        format!("{birth:?}"),
    ];

    for line in printed {
        assert!(!line.contains(canary), "{line}");
        assert!(!line.contains(did), "{line}");
    }
}
