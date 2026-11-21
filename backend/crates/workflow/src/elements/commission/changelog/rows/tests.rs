use super::*;
use crate::elements::commission::{
    CommissionState, DeadlineStatus, DirectionStatus, ElementId, LateFacts, LifecycleStep, NewSlot,
    SurfaceAddress, TabId, clear_deadline, clear_delayed, clear_direction, clear_dispute,
    move_lifecycle, owes_late, raise_dispute, rate_maturity, schedule_deadline, set_delayed,
    set_direction, test_commission,
};
use identity::elements::user::UserId;
use kernel::elements::{
    did::Did,
    maturity::{Maturity, MaturityRating},
};

fn actor() -> UserId {
    UserId::from(Did::from("did:plc:actor".to_owned()))
}

/// An instant written as RFC 3339.
fn instant(text: &str) -> DateTimeUtc {
    text.parse::<DateTimeUtc>().expect("an RFC 3339 instant")
}

/// The earlier deadline the tests hold.
fn march() -> DateTimeUtc {
    instant("2026-03-01T12:00:00Z")
}

/// The later deadline the tests move to.
fn april() -> DateTimeUtc {
    instant("2026-04-01T12:00:00Z")
}

/// When the acts happen: after both deadlines.
fn may() -> DateTimeUtc {
    instant("2026-05-01T12:00:00Z")
}

/// When an act happens before either deadline.
fn february() -> DateTimeUtc {
    instant("2026-02-01T12:00:00Z")
}

/// The element id the declared Slot in the tests is born as.
const DECLARED_ELEMENT: &str = "0192f6f0-0000-7000-8000-00000000510d";

/// A Slot's declaration under [`DECLARED_ELEMENT`], with `notes`.
fn declaration(notes: Option<&str>) -> SlotDeclaration {
    let address = SurfaceAddress::new(
        TabId::from(uuid::Uuid::now_v7()),
        "content".parse().expect("a surface name"),
    );
    let slot = NewSlot {
        address,
        notes: notes.map(str::to_owned),
    };
    let element = DECLARED_ELEMENT
        .parse::<ElementId>()
        .expect("an element id");
    SlotDeclaration { element, slot }
}

/// The state of an Active commission with `deadline`, the stored `flag` and `direction`.
fn active(
    deadline: Option<DateTimeUtc>,
    flag: Option<DeadlineStatus>,
    direction: Option<DirectionStatus>,
) -> CommissionState {
    let mut commission = test_commission::stored(LifecycleStep::Active, deadline, flag);
    commission.direction_status = direction;
    CommissionState::from(&commission)
}

/// Every gated entry, each built by its own constructor from a change its rule passed, beside
/// the literal JSON it stores and its kind's token.
fn gated_entries(commission: CommissionId) -> Vec<(NewChangelogEntry, &'static str, Value)> {
    let at = february();

    let created = NewChangelogEntry::created(commission, actor(), at);
    let renamed = NewChangelogEntry::renamed(commission, actor(), at);

    let moved = move_lifecycle(&active(None, None, None), LifecycleStep::Completed)
        .expect("active → completed passes")
        .expect("a real move");
    let lifecycle_moved = NewChangelogEntry::lifecycle_moved(commission, actor(), &moved, at);

    let from_draft = test_commission::at(LifecycleStep::Draft);
    let dispute = raise_dispute(&CommissionState::from(&from_draft), at).expect("a raise passes");
    let disputed = NewChangelogEntry::disputed(commission, actor(), &dispute, at);
    let held = test_commission::disputed(LifecycleStep::Draft, at, None);
    let clear = clear_dispute(&CommissionState::from(&held)).expect("a clear passes");
    let dispute_cleared = NewChangelogEntry::dispute_cleared(commission, actor(), &clear, at);

    let first_direction =
        set_direction(&active(None, None, None), DirectionStatus::WaitingForInput)
            .expect("a real set");
    let direction_first =
        NewChangelogEntry::direction_set(commission, actor(), &first_direction, at);
    let replaced = set_direction(
        &active(None, None, Some(DirectionStatus::WaitingForInput)),
        DirectionStatus::ChangesRequested,
    )
    .expect("a real set");
    let direction_replaced = NewChangelogEntry::direction_set(commission, actor(), &replaced, at);
    let cleared = clear_direction(&active(
        None,
        None,
        Some(DirectionStatus::WaitingForApproval),
    ))
    .expect("a real clear");
    let direction_cleared = NewChangelogEntry::direction_cleared(commission, actor(), &cleared, at);

    let first_deadline = schedule_deadline(&active(None, None, None), march().into())
        .expect("passes")
        .expect("a real set");
    let deadline_first = NewChangelogEntry::deadline_set(commission, actor(), &first_deadline, at);
    let extended = schedule_deadline(&active(Some(march()), None, None), april().into())
        .expect("passes")
        .expect("a real set");
    let deadline_extended = NewChangelogEntry::deadline_set(commission, actor(), &extended, at);
    let pulled_in = schedule_deadline(&active(Some(april()), None, None), march().into())
        .expect("passes")
        .expect("a real set");
    let deadline_pulled_in = NewChangelogEntry::deadline_set(commission, actor(), &pulled_in, at);
    let deadline_gone = clear_deadline(&active(Some(march()), None, None))
        .expect("passes")
        .expect("a real clear");
    let deadline_cleared =
        NewChangelogEntry::deadline_cleared(commission, actor(), &deadline_gone, at);

    let raised = set_delayed(&active(Some(march()), None, None), at)
        .expect("passes")
        .expect("a real set");
    let delayed_set = NewChangelogEntry::delayed_set(commission, actor(), &raised, at);
    let lowered = clear_delayed(
        &active(Some(march()), Some(DeadlineStatus::Delayed), None),
        at,
    )
    .expect("passes")
    .expect("a real clear");
    let delayed_cleared = NewChangelogEntry::delayed_cleared(commission, actor(), &lowered, at);

    let late_facts = LateFacts {
        state: active(Some(march()), Some(DeadlineStatus::Delayed), None),
        owed: true,
    };
    let mark = owes_late(&late_facts, may()).expect("a late entry is owed");
    let late = NewChangelogEntry::late(commission, &mark, may());

    let nudity = Maturity {
        rating: MaturityRating::Nudity,
        graphic: true,
    };
    let first_rating = rate_maturity(&active(None, None, None), nudity).expect("a real rating");
    let maturity_rated = NewChangelogEntry::maturity_rated(commission, actor(), &first_rating, at);
    let chosen = NewCommission {
        maturity: Some(nudity),
        deadline: None,
    };
    let rated_at_create = NewChangelogEntry::rated_at_create(commission, actor(), &chosen, at)
        .expect("a create given a rating records it");

    let slot_declared =
        NewChangelogEntry::slot_declared(commission, actor(), &declaration(Some("notes")), at);

    vec![
        (created, "created", json!({})),
        (renamed, "renamed", json!({})),
        (
            lifecycle_moved,
            "lifecycle_moved",
            json!({ "from": "active", "to": "completed" }),
        ),
        (
            disputed,
            "lifecycle_moved",
            json!({ "from": "draft", "to": "disputed" }),
        ),
        (
            dispute_cleared,
            "lifecycle_moved",
            json!({ "from": "disputed", "to": "draft" }),
        ),
        (
            direction_first,
            "status_changed",
            json!({ "from": null, "to": "waiting_for_input" }),
        ),
        (
            direction_replaced,
            "status_changed",
            json!({ "from": "waiting_for_input", "to": "changes_requested" }),
        ),
        (
            direction_cleared,
            "status_changed",
            json!({ "from": "waiting_for_approval", "to": null }),
        ),
        (
            deadline_first,
            "deadline_set",
            json!({ "from": null, "to": "2026-03-01T12:00:00Z" }),
        ),
        (
            deadline_extended,
            "deadline_extended",
            json!({ "from": "2026-03-01T12:00:00Z", "to": "2026-04-01T12:00:00Z" }),
        ),
        (
            deadline_pulled_in,
            "deadline_set",
            json!({ "from": "2026-04-01T12:00:00Z", "to": "2026-03-01T12:00:00Z" }),
        ),
        (
            deadline_cleared,
            "deadline_set",
            json!({ "from": "2026-03-01T12:00:00Z", "to": null }),
        ),
        (
            delayed_set,
            "delayed",
            json!({ "from": null, "to": "delayed", "deadline": "2026-03-01T12:00:00Z" }),
        ),
        (
            delayed_cleared,
            "delayed",
            json!({ "from": "delayed", "to": null, "deadline": "2026-03-01T12:00:00Z" }),
        ),
        (
            late,
            "late",
            json!({ "deadline": "2026-03-01T12:00:00Z", "from": "delayed" }),
        ),
        (
            maturity_rated,
            "maturity_rated",
            json!({ "rating": "nudity", "graphic": true }),
        ),
        (
            rated_at_create,
            "maturity_rated",
            json!({ "rating": "nudity", "graphic": true }),
        ),
        (
            slot_declared,
            "slot_declared",
            json!({ "slot": DECLARED_ELEMENT }),
        ),
    ]
}

// Each legacy kind's typed payload is the JSON today's stored rows hold, literally.
#[test]
fn each_legacy_kind_s_typed_payload_is_its_stored_shape() {
    let commission = CommissionId::mint();
    let legacy = gated_entries(commission)
        .into_iter()
        .filter(|(_, token, _)| {
            !matches!(
                *token,
                "lifecycle_moved" | "maturity_rated" | "slot_declared"
            )
        });

    for (entry, token, stored_shape) in legacy {
        assert_eq!(<&'static str>::from(entry.kind()), token);
        assert_eq!(entry.payload(), &stored_shape, "{token}");
        assert_eq!(
            entry.payload().to_string(),
            stored_shape.to_string(),
            "{token}"
        );
    }
}

// A lifecycle move, a dispute raised and a dispute cleared each store their two step tokens and
// nothing else, as literal JSON; a rating stores its rating token and its Graphic flag and
// nothing else, with no note, the held rating not among them.
#[test]
fn each_new_kind_s_stored_shape_is_its_literal_json() {
    let moved = move_lifecycle(&active(None, None, None), LifecycleStep::Batched)
        .expect("active → batched passes")
        .expect("a real move");
    let entry = NewChangelogEntry::lifecycle_moved(CommissionId::mint(), actor(), &moved, may());
    let dispute = raise_dispute(&active(None, None, None), may()).expect("a raise passes");
    let raised = NewChangelogEntry::disputed(CommissionId::mint(), actor(), &dispute, may());
    let held = test_commission::disputed(LifecycleStep::Batched, may(), None);
    let clear = clear_dispute(&CommissionState::from(&held)).expect("a clear passes");
    let cleared = NewChangelogEntry::dispute_cleared(CommissionId::mint(), actor(), &clear, may());

    let lifecycle_moved = json!({ "from": "active", "to": "batched" });
    assert_eq!(entry.kind(), ChangelogEntryKind::LifecycleMoved);
    assert_eq!(entry.payload(), &lifecycle_moved);
    assert_eq!(entry.actor_id(), Some(&actor()));
    let stored = [
        (raised, r#"{"from":"active","to":"disputed"}"#),
        (cleared, r#"{"from":"disputed","to":"batched"}"#),
    ];
    for (entry, literal) in stored {
        assert_eq!(entry.kind(), ChangelogEntryKind::LifecycleMoved);
        assert_eq!(entry.payload().to_string(), literal);
        assert_eq!(entry.note_text(), None);
        assert_eq!(entry.actor_id(), Some(&actor()));
    }

    let safe = Maturity {
        rating: MaturityRating::Safe,
        graphic: false,
    };
    let adult = Maturity {
        rating: MaturityRating::Adult,
        graphic: true,
    };
    let mut rated_safe = test_commission::at(LifecycleStep::Active);
    rated_safe.maturity = Some(safe);
    let re_rated =
        rate_maturity(&CommissionState::from(&rated_safe), adult).expect("a real rating");
    let rating = NewChangelogEntry::maturity_rated(CommissionId::mint(), actor(), &re_rated, may());

    // serde_json writes an object's keys in order, so the literal reads `graphic` first.
    assert_eq!(rating.kind(), ChangelogEntryKind::MaturityRated);
    assert_eq!(
        rating.payload().to_string(),
        r#"{"graphic":true,"rating":"adult"}"#
    );
    assert_eq!(rating.note_text(), None);
    assert_eq!(rating.actor_id(), Some(&actor()));
}

// A declared Slot's entry stores its element id and nothing else: no title, no notes.
#[test]
fn a_slot_declared_entry_stores_only_its_slot() {
    let canary = "Canary notes 51d0";
    let entry = NewChangelogEntry::slot_declared(
        CommissionId::mint(),
        actor(),
        &declaration(Some(canary)),
        may(),
    );

    let literal = format!("{{\"slot\":\"{DECLARED_ELEMENT}\"}}");
    assert_eq!(entry.kind(), ChangelogEntryKind::SlotDeclared);
    assert_eq!(entry.payload().to_string(), literal);
    assert_eq!(entry.actor_id(), Some(&actor()));
    assert_eq!(entry.note_text(), None);
    assert!(!format!("{entry:?}").contains(canary));
}

// No gated entry carries a note, whichever constructor built it.
#[test]
fn every_gated_entry_has_no_note() {
    for (entry, token, _) in gated_entries(CommissionId::mint()) {
        assert_eq!(entry.note_text(), None, "{token}");
    }
}

// Every gated entry but the system's Late mark names its actor; the Late mark names none.
#[test]
fn constructors_set_the_actor_arm() {
    for (entry, token, _) in gated_entries(CommissionId::mint()) {
        let expected_actor = (token != "late").then(actor);
        assert_eq!(entry.actor_id(), expected_actor.as_ref(), "{token}");
    }

    let commission = CommissionId::mint();
    let now = may();
    let note = NewChangelogEntry::note(commission, actor(), "hi".to_owned(), now);
    assert_eq!(note.kind(), ChangelogEntryKind::Note);
    assert_eq!(note.note_text(), Some("hi"));
    assert_eq!(note.actor_id(), Some(&actor()));

    let open = NewChangelogEntry::event(
        commission,
        OpenKind::Archived,
        actor(),
        json!({ "title": "Old sketch" }),
        now,
    );
    let attached = NewChangelogEntry::from(open.with_note("love the colors!".to_owned()));
    assert_eq!(attached.kind(), ChangelogEntryKind::Archived);
    assert_eq!(attached.actor_id(), Some(&actor()));
    assert_eq!(attached.note_text(), Some("love the colors!"));
}

// A create entry's constructor leaves no room for a title or a note.
#[test]
fn a_create_entry_carries_no_title_and_no_note() {
    let commission = CommissionId::mint();
    let now = may();

    let created = NewChangelogEntry::created(commission, actor(), now);

    let empty_payload = json!({});
    assert_eq!(created.kind(), ChangelogEntryKind::Created);
    assert_eq!(created.commission_id(), commission);
    assert_eq!(created.actor_id(), Some(&actor()));
    assert_eq!(
        created.payload(),
        &empty_payload,
        "the payload names no title"
    );
    assert_eq!(created.note_text(), None, "a create carries no note");
    assert_eq!(created.created_at(), now);
}

// A rename entry is built only by its constructor, which leaves no room for text.
#[test]
fn a_rename_entry_carries_no_title_and_no_note() {
    let renamed = NewChangelogEntry::renamed(CommissionId::mint(), actor(), may());

    let empty_payload = json!({});
    assert_eq!(renamed.kind(), ChangelogEntryKind::Renamed);
    assert_eq!(renamed.actor_id(), Some(&actor()));
    assert_eq!(
        renamed.payload(),
        &empty_payload,
        "the payload names no title"
    );
    assert_eq!(renamed.note_text(), None, "a rename carries no note");
}

// A create given a rating records it as a later rating would: kind `maturity_rated`, its rating
// token and Graphic flag as literal JSON, by its creator, at its time, with no note; a create
// given no rating records none.
#[test]
fn a_create_s_rating_is_recorded_in_the_rating_s_literal_shape() {
    let commission = CommissionId::mint();
    let adult = Maturity {
        rating: MaturityRating::Adult,
        graphic: true,
    };
    let rated = NewCommission {
        maturity: Some(adult),
        deadline: Some(april().into()),
    };
    let unrated = NewCommission {
        maturity: None,
        deadline: Some(april().into()),
    };

    let entry = NewChangelogEntry::rated_at_create(commission, actor(), &rated, may())
        .expect("a rating was given");
    let none = NewChangelogEntry::rated_at_create(commission, actor(), &unrated, may());

    let later = rate_maturity(&active(None, None, None), adult).expect("a real rating");
    let later_rating = NewChangelogEntry::maturity_rated(commission, actor(), &later, may());
    // serde_json writes an object's keys in order, so the literal reads `graphic` first.
    assert_eq!(entry.kind(), ChangelogEntryKind::MaturityRated);
    assert_eq!(
        entry.payload().to_string(),
        r#"{"graphic":true,"rating":"adult"}"#
    );
    assert_eq!(entry.payload(), later_rating.payload());
    assert_eq!(entry.commission_id(), commission);
    assert_eq!(entry.actor_id(), Some(&actor()));
    assert_eq!(entry.created_at(), may());
    assert_eq!(entry.note_text(), None);
    assert!(none.is_none(), "{none:?}");
}
