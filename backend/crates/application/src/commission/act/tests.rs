use domain::elements::{commission::NewCommission, level::Level};

use super::*;
use crate::den::{
    DenType, TopLevel,
    act::{ActShape, Anchor},
};

/// The create act for a commission titled `title`, choosing nothing.
fn create(title: &str) -> CommissionAct {
    let act = create::Act {
        title: title.parse().expect("a valid title"),
        chosen: NewCommission {
            maturity: None,
            deadline: None,
        },
    };
    CommissionAct::Create(act)
}

#[test]
fn create_needs_no_role() {
    let spec = create("A piece").spec();

    assert_eq!(spec.needs, None);
    assert_eq!(spec.anchor, Anchor::TopLevel(TopLevel::Commission));
    assert_eq!(spec.den_type, DenType::Commission);
}

#[test]
fn a_create_makes_six_private_entries_named_by_their_words() {
    let spec = create("A piece").spec();

    let ActShape::Create {
        name,
        level,
        entries,
    } = spec.shape
    else {
        panic!("a create's shape is a create");
    };
    let made: Vec<(&str, DenType, Option<Level>)> = entries
        .iter()
        .map(|entry| (entry.word, entry.den_type, entry.level))
        .collect();
    let expected = [
        ("attachments", DenType::CommissionAttachments, None),
        ("products", DenType::CommissionProducts, None),
        ("slots", DenType::CommissionSlots, None),
        ("changelog", DenType::CommissionChangelog, None),
        ("participants", DenType::CommissionParticipants, None),
        ("brief", DenType::CommissionBrief, None),
    ];
    assert_eq!(name.as_ref(), "A piece");
    assert_eq!(level, None);
    assert_eq!(made, expected);
}

#[test]
fn an_act_s_debug_prints_no_title() {
    let printed = format!("{:?}", create("A secret piece"));

    assert_eq!(printed, "Create");
}
