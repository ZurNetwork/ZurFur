use adapter_mem::fixture::{bob, untitled};
use chrono::Utc;
use domain::{
    datetime::DateTimeUtc,
    elements::commission::{ChangelogEntryKind, LifecycleStep},
};

use crate::{
    commission::CommissionError,
    den::test_acts::{FieldCall, entries_of, field_call, fields_of},
    test_world,
};

/// The deadline these tests set.
fn march() -> DateTimeUtc {
    "2099-03-01T00:00:00Z".parse().expect("a timestamp")
}

// Bob, a Participant, clears Untitled's deadline: one `deadline_set` entry naming the deadline
// cleared and none set; a second clear finds nothing held and records nothing.
#[tokio::test]
async fn a_participant_clears_the_deadline_once() {
    let world = test_world::world().await;
    let dated = FieldCall::SetDeadline(march());
    field_call(&world.ports, &bob(), untitled(), dated, Utc::now())
        .await
        .expect("bob sets the deadline");
    let before = entries_of(&world.ports, untitled()).await.len();

    for _ in 0..2 {
        field_call(
            &world.ports,
            &bob(),
            untitled(),
            FieldCall::ClearDeadline,
            Utc::now(),
        )
        .await
        .expect("bob clears the deadline");
    }

    let stream = entries_of(&world.ports, untitled()).await;
    let [entry] = &stream[before..] else {
        panic!("one entry: {} of them", stream.len() - before);
    };
    let payload = serde_json::json!({ "from": "2099-03-01T00:00:00Z", "to": null });
    assert_eq!(entry.kind, ChangelogEntryKind::DeadlineSet);
    assert_eq!(entry.actor_id, Some(bob()));
    assert_eq!(entry.payload, payload);
    assert_eq!(fields_of(&world.ports, untitled()).await.deadline(), None);
}

// While Disputed the deadline is frozen: its clear is refused as `commission_disputed`, and
// nothing lands.
#[tokio::test]
async fn a_disputed_commission_s_deadline_clear_is_refused() {
    let world = test_world::world().await;
    let dated = FieldCall::SetDeadline(march());
    field_call(&world.ports, &bob(), untitled(), dated, Utc::now())
        .await
        .expect("bob sets the deadline");
    world
        .backend
        .seed_lifecycle_step(untitled(), LifecycleStep::Disputed);
    let before = entries_of(&world.ports, untitled()).await.len();

    let refused = field_call(
        &world.ports,
        &bob(),
        untitled(),
        FieldCall::ClearDeadline,
        Utc::now(),
    )
    .await;

    assert!(
        matches!(refused, Err(CommissionError::CommissionDisputed)),
        "{refused:?}"
    );
    assert_eq!(entries_of(&world.ports, untitled()).await.len(), before);
    assert_eq!(
        fields_of(&world.ports, untitled()).await.deadline(),
        Some(march())
    );
}
