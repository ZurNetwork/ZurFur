use adapter_mem::fixture::untitled;
use domain::elements::{
    commission::{CommissionRole, NewCommission},
    level::Level,
};

use super::*;
use crate::{
    commission::den::CommissionPlace,
    den::{
        DenType, Place, TopLevel,
        act::{ActShape, Anchor},
        needs,
    },
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

/// The rename act giving "Untitled" the name `title`.
fn rename(title: &str) -> CommissionAct {
    let act = rename::Act {
        commission: untitled(),
        title: title.parse().expect("a valid title"),
    };
    CommissionAct::Rename(act)
}

// Each act's needed role is its capability marker's, never written apart: none for a create,
// the Owner for a rename.
#[test]
fn create_needs_no_role_and_rename_needs_the_owner() {
    let created = create("A piece").spec();
    let renamed = rename("A new name").spec();

    assert_eq!(created.needs, None);
    assert_eq!(created.needs, needs::<create::Act>());
    assert_eq!(created.anchor, Anchor::TopLevel(TopLevel::Commission));
    assert_eq!(created.den_type, DenType::Commission);
    let untitled_place = Place::Commission(CommissionPlace::Commission(untitled()));
    assert_eq!(renamed.needs, Some(CommissionRole::Owner));
    assert_eq!(renamed.needs, needs::<rename::Act>());
    assert_eq!(renamed.anchor, Anchor::Thing(untitled_place));
    assert_eq!(renamed.den_type, DenType::Commission);
}

#[test]
fn a_rename_s_shape_is_its_new_name() {
    let spec = rename("A new name").spec();

    let ActShape::Rename { name } = spec.shape else {
        panic!("a rename's shape is a rename");
    };
    assert_eq!(name.as_ref(), "A new name");
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
    let printed = [
        format!("{:?}", create("A secret piece")),
        format!("{:?}", rename("A secret name")),
    ];

    assert_eq!(printed, ["Create", "Rename"]);
}

// A lifecycle move needs any Participant, read from its capability marker, and changes the
// commission's fields on its own node; its Debug prints the variant alone.
#[test]
fn a_lifecycle_move_needs_a_participant_and_is_a_change() {
    let act = move_to::Act {
        commission: untitled(),
        to: domain::elements::commission::LifecycleStep::Active,
    };
    let moved = CommissionAct::MoveLifecycle(act);

    let spec = moved.spec();

    let untitled_place = Place::Commission(CommissionPlace::Commission(untitled()));
    assert_eq!(spec.needs, Some(CommissionRole::Participant));
    assert_eq!(spec.needs, needs::<move_to::Act>());
    assert_eq!(spec.anchor, Anchor::Thing(untitled_place));
    assert_eq!(spec.den_type, DenType::Commission);
    assert_eq!(spec.shape, ActShape::Change);
    assert_eq!(format!("{moved:?}"), "MoveLifecycle");
}

// Each direction, deadline and Delayed act needs any Participant, read from its capability
// marker, and changes the commission's fields on its own node; its Debug prints the variant
// alone, never a value it carries.
#[test]
fn each_field_act_needs_a_participant_and_is_a_change() {
    let deadline = "2099-03-01T00:00:00Z".parse().expect("a timestamp");
    let commission = untitled();
    let acts = [
        CommissionAct::SetDirection(direction::set::Act {
            commission,
            to: domain::elements::commission::DirectionStatus::WaitingForInput,
        }),
        CommissionAct::ClearDirection(direction::clear::Act { commission }),
        CommissionAct::SetDeadline(deadline::set::Act {
            commission,
            deadline,
        }),
        CommissionAct::ClearDeadline(deadline::clear::Act { commission }),
        CommissionAct::SetDelayed(deadline::status::set::Act { commission }),
        CommissionAct::ClearDelayed(deadline::status::clear::Act { commission }),
    ];

    let untitled_place = Place::Commission(CommissionPlace::Commission(untitled()));
    for act in &acts {
        let spec = act.spec();

        assert_eq!(spec.needs, Some(CommissionRole::Participant), "{act:?}");
        assert_eq!(
            spec.anchor,
            Anchor::Thing(untitled_place.clone()),
            "{act:?}"
        );
        assert_eq!(spec.den_type, DenType::Commission, "{act:?}");
        assert_eq!(spec.shape, ActShape::Change, "{act:?}");
    }
    let printed: Vec<String> = acts.iter().map(|act| format!("{act:?}")).collect();
    let variants = [
        "SetDirection",
        "ClearDirection",
        "SetDeadline",
        "ClearDeadline",
        "SetDelayed",
        "ClearDelayed",
    ];
    assert_eq!(printed, variants);
    assert_eq!(
        needs::<direction::set::Act>(),
        Some(CommissionRole::Participant)
    );
    assert_eq!(
        needs::<deadline::status::clear::Act>(),
        Some(CommissionRole::Participant)
    );
}
