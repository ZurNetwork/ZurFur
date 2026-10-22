use adapter_mem::fixture::{alice, untitled};
use chrono::Utc;
use domain::elements::commission::{LifecycleStep, move_lifecycle};

use super::*;
use crate::den::test_acts::state_at;

/// The move from Draft to Active, as its rule checks it.
fn draft_to_active() -> LifecycleMove {
    let draft = state_at(LifecycleStep::Draft);
    move_lifecycle(&draft, LifecycleStep::Active)
        .expect("Draft moves to Active")
        .expect("a real move")
}

// Each bare gated entry, built on a commission, by an actor, at a time, carries its own kind,
// that commission, actor and time, an empty JSON object as its payload and no note: the
// entries the gate stores stay as they are.
#[test]
fn each_gated_entry_builds_its_own_kind_with_an_empty_payload_and_no_note() {
    let now = Utc::now();
    let gated: [(GatedEntry<()>, ChangelogEntryKind); 2] = [
        (GatedEntry::CREATED, ChangelogEntryKind::Created),
        (GatedEntry::RENAMED, ChangelogEntryKind::Renamed),
    ];
    let empty_payload = serde_json::json!({});

    for (entry, kind) in gated {
        let built = entry
            .construct(untitled(), alice(), None, now)
            .expect("a bare constructor needs no change");

        assert!(entry.builds(kind), "{kind}");
        assert_eq!(built.kind(), kind);
        assert_eq!(built.commission_id(), untitled(), "{kind}");
        assert_eq!(built.actor_id(), Some(&alice()), "{kind}");
        assert_eq!(built.created_at(), now, "{kind}");
        assert_eq!(built.payload(), &empty_payload, "{kind}");
        assert_eq!(built.note_text(), None, "{kind}");
    }
}

// The lifecycle move's entry is built from the move it is handed, and from nothing else.
#[test]
fn the_lifecycle_entry_is_built_from_the_move_it_is_handed() {
    let now = Utc::now();
    let moved = draft_to_active();
    let gated = GatedEntry::LIFECYCLE_MOVED;

    let built = gated
        .construct(untitled(), alice(), Some(&moved), now)
        .expect("a move was handed");
    let unbuilt = gated.construct(untitled(), alice(), None, now);

    let payload = serde_json::json!({ "from": "draft", "to": "active" });
    assert_eq!(built.kind(), ChangelogEntryKind::LifecycleMoved);
    assert!(gated.builds(ChangelogEntryKind::LifecycleMoved));
    assert!(!gated.builds(ChangelogEntryKind::Renamed));
    assert_eq!(built.payload(), &payload);
    assert_eq!(built.note_text(), None);
    assert!(unbuilt.is_none(), "{unbuilt:?}");
}

#[test]
fn an_on_change_rule_names_its_entry_only_when_the_act_changed_something() {
    let always = || EntryRule::<()>::Always(GatedEntry::CREATED);
    let on_change = || EntryRule::<()>::OnChange(GatedEntry::RENAMED);
    let kind = |gated: Option<GatedEntry<()>>| {
        gated.and_then(|gated| {
            let built = gated.construct(untitled(), alice(), None, Utc::now())?;
            Some(built.kind())
        })
    };

    let kinds = [
        kind(always().entry(true)),
        kind(always().entry(false)),
        kind(on_change().entry(true)),
        kind(on_change().entry(false)),
    ];

    let expected = [
        Some(ChangelogEntryKind::Created),
        Some(ChangelogEntryKind::Created),
        Some(ChangelogEntryKind::Renamed),
        None,
    ];
    assert_eq!(kinds, expected);
}
