use adapter_mem::fixture::{bob, untitled};
use chrono::Utc;
use domain::elements::commission::{ChangelogEntryKind, DirectionStatus};

use crate::{
    commission::CommissionError,
    common_error::{CommonError, NotFoundEntity},
    den::test_acts::{FieldCall, entries_of, field_call, fields_of},
    test_world,
};

// Bob, a Participant, clears Untitled's direction status: one `status_changed` entry naming the
// status cleared and none set.
#[tokio::test]
async fn a_participant_clears_the_direction_status() {
    let world = test_world::world().await;
    let waiting = FieldCall::SetDirection(DirectionStatus::WaitingForInput);
    field_call(&world.ports, &bob(), untitled(), waiting, Utc::now())
        .await
        .expect("bob sets the status");
    let before = entries_of(&world.ports, untitled()).await.len();

    field_call(
        &world.ports,
        &bob(),
        untitled(),
        FieldCall::ClearDirection,
        Utc::now(),
    )
    .await
    .expect("bob clears the status");

    let stream = entries_of(&world.ports, untitled()).await;
    let [entry] = &stream[before..] else {
        panic!("one entry: {} of them", stream.len() - before);
    };
    let payload = serde_json::json!({ "from": "waiting_for_input", "to": null });
    assert_eq!(entry.kind, ChangelogEntryKind::StatusChanged);
    assert_eq!(entry.actor_id, Some(bob()));
    assert_eq!(entry.payload, payload);
    assert_eq!(
        fields_of(&world.ports, untitled()).await.direction_status(),
        None
    );
}

// Clearing no status writes nothing and records nothing; a stranger's clear is the same
// not-found as a made-up id's.
#[tokio::test]
async fn clearing_nothing_writes_nothing_and_a_stranger_is_not_found() {
    let world = test_world::world().await;
    let before = entries_of(&world.ports, untitled()).await.len();

    field_call(
        &world.ports,
        &bob(),
        untitled(),
        FieldCall::ClearDirection,
        Utc::now(),
    )
    .await
    .expect("a clear of nothing lands");
    let stranger = test_world::stranger();
    let unseen = field_call(
        &world.ports,
        &stranger,
        untitled(),
        FieldCall::ClearDirection,
        Utc::now(),
    )
    .await;
    let made_up = field_call(
        &world.ports,
        &bob(),
        test_world::absent_commission(),
        FieldCall::ClearDirection,
        Utc::now(),
    )
    .await;

    assert_eq!(entries_of(&world.ports, untitled()).await.len(), before);
    for (label, cleared) in [("a stranger", unseen), ("a made-up id", made_up)] {
        assert!(
            matches!(
                cleared,
                Err(CommissionError::CommonError(CommonError::NotFound(
                    NotFoundEntity::Commission
                )))
            ),
            "{label}: {cleared:?}"
        );
    }
}
