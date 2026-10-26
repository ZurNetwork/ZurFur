use adapter_mem::fixture::{alice, untitled};
use chrono::Utc;
use domain::elements::{
    commission::{LifecycleStep, move_lifecycle},
    maturity::{Maturity, MaturityRating},
};

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
            .construct(untitled(), Some(&alice()), None, now)
            .expect("a bare constructor needs no change")
            .expect("a bare constructor always builds");

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
        .construct(untitled(), Some(&alice()), Some(&moved), now)
        .expect("a move was handed")
        .expect("a change constructor always builds");
    let unbuilt = gated.construct(untitled(), Some(&alice()), None, now);

    let payload = serde_json::json!({ "from": "draft", "to": "active" });
    assert_eq!(built.kind(), ChangelogEntryKind::LifecycleMoved);
    assert!(gated.builds(ChangelogEntryKind::LifecycleMoved));
    assert!(!gated.builds(ChangelogEntryKind::Renamed));
    assert_eq!(built.payload(), &payload);
    assert_eq!(built.note_text(), None);
    assert!(matches!(unbuilt, Err(Unbuildable)), "{unbuilt:?}");
}

#[test]
fn an_on_change_rule_names_its_entry_only_when_the_act_changed_something() {
    let always = || EntryRule::<()>::Always(GatedEntry::CREATED);
    let on_change = || EntryRule::<()>::OnChange(GatedEntry::RENAMED);
    let kind = |planned: Option<Planned<()>>| {
        planned.and_then(|Planned { first, then }| {
            assert!(then.is_none(), "a one-entry rule plans no second entry");
            let built = first
                .construct(untitled(), Some(&alice()), None, Utc::now())
                .ok()??;
            Some(built.kind())
        })
    };

    let kinds = [
        kind(always().planned(true)),
        kind(always().planned(false)),
        kind(on_change().planned(true)),
        kind(on_change().planned(false)),
    ];

    let expected = [
        Some(ChangelogEntryKind::Created),
        Some(ChangelogEntryKind::Created),
        Some(ChangelogEntryKind::Renamed),
        None,
    ];
    assert_eq!(kinds, expected);
}

/// What a create chose: `maturity`, and no deadline.
fn chosen(maturity: Option<Maturity>) -> NewCommission {
    NewCommission {
        maturity,
        deadline: None,
    }
}

// The create's rule plans its `created` entry whatever changed, then its rating's: built from a
// rating the create chose, in the rating's literal shape, and nothing from a create that chose
// none.
#[test]
fn the_create_s_rule_plans_its_created_entry_then_its_rating_s() {
    let now = Utc::now();
    let adult = Maturity {
        rating: MaturityRating::Adult,
        graphic: true,
    };
    let rated = chosen(Some(adult));
    let unrated = chosen(None);
    let rule =
        || EntryRule::<NewCommission>::AlwaysThen(GatedEntry::CREATED, GatedEntry::RATED_AT_CREATE);

    for changed in [true, false] {
        let Some(Planned { first, then }) = rule().planned(changed) else {
            panic!("the create's rule plans its entries whatever changed");
        };
        let then = then.expect("the create's rule plans a second entry");
        let created = first
            .construct(untitled(), Some(&alice()), Some(&rated), now)
            .expect("the create's entry needs no change")
            .expect("the create's entry always builds");
        let rating = then
            .construct(untitled(), Some(&alice()), Some(&rated), now)
            .expect("a create's choice was handed")
            .expect("a rating was chosen");
        let no_rating = then
            .construct(untitled(), Some(&alice()), Some(&unrated), now)
            .expect("a create's choice was handed");
        let unhanded = then.construct(untitled(), Some(&alice()), None, now);

        assert_eq!(created.kind(), ChangelogEntryKind::Created);
        assert_eq!(rating.kind(), ChangelogEntryKind::MaturityRated);
        assert!(then.builds(ChangelogEntryKind::MaturityRated));
        assert!(!then.builds(ChangelogEntryKind::Created));
        assert_eq!(
            rating.payload().to_string(),
            r#"{"graphic":true,"rating":"adult"}"#
        );
        assert_eq!(rating.actor_id(), Some(&alice()));
        assert_eq!(rating.created_at(), now);
        assert_eq!(rating.note_text(), None);
        assert!(no_rating.is_none(), "{no_rating:?}");
        assert!(matches!(unhanded, Err(Unbuildable)), "{unhanded:?}");
    }
}
