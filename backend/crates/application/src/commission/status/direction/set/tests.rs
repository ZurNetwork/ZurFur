use std::sync::Arc;

use adapter_mem::fixture::{bob, untitled};
use chrono::Utc;
use domain::elements::{
    commission::{ChangelogEntryKind, CommissionId, DirectionStatus, LifecycleStep},
    user::UserId,
};
use test_support::recording::{CallLog, RecordingDatabase};

use crate::{
    Ports,
    commission::{CommissionError, CommissionResult},
    common_error::{CommonError, NotFoundEntity},
    den::test_acts::{FieldCall, entries_of, field_call, fields_of},
    test_world,
};

/// Records `begin`, `commit` and `rollback` on `ports`' database from here on.
fn record_units(ports: &mut Ports) -> CallLog {
    let log = CallLog::default();
    let recording = RecordingDatabase::new(ports.database.clone(), log.clone());
    ports.database = Arc::new(recording);
    log
}

/// `actor`'s set of `commission`'s direction status to `to`, through the use case.
async fn set_by(
    ports: &Ports,
    actor: &UserId,
    commission: CommissionId,
    to: DirectionStatus,
) -> CommissionResult<()> {
    field_call(
        ports,
        actor,
        commission,
        FieldCall::SetDirection(to),
        Utc::now(),
    )
    .await
}

// Bob, a Participant, sets Untitled's direction status: one `status_changed` entry by him,
// naming no status held and the one set.
#[tokio::test]
async fn a_participant_sets_the_direction_status() {
    let world = test_world::world().await;
    let before = entries_of(&world.ports, untitled()).await.len();

    set_by(
        &world.ports,
        &bob(),
        untitled(),
        DirectionStatus::WaitingForInput,
    )
    .await
    .expect("bob sets the status");

    let stream = entries_of(&world.ports, untitled()).await;
    let [entry] = &stream[before..] else {
        panic!("one entry: {} of them", stream.len() - before);
    };
    let payload = serde_json::json!({ "from": null, "to": "waiting_for_input" });
    assert_eq!(entry.kind, ChangelogEntryKind::StatusChanged);
    assert_eq!(entry.actor_id, Some(bob()));
    assert_eq!(entry.payload, payload);
    assert_eq!(
        fields_of(&world.ports, untitled()).await.direction_status(),
        Some(DirectionStatus::WaitingForInput)
    );
}

// Setting the status held writes nothing and records nothing.
#[tokio::test]
async fn setting_the_held_status_writes_nothing() {
    let world = test_world::world().await;
    set_by(
        &world.ports,
        &bob(),
        untitled(),
        DirectionStatus::WaitingForInput,
    )
    .await
    .expect("bob sets the status");
    let before = entries_of(&world.ports, untitled()).await.len();

    set_by(
        &world.ports,
        &bob(),
        untitled(),
        DirectionStatus::WaitingForInput,
    )
    .await
    .expect("a set of the held status lands");

    assert_eq!(entries_of(&world.ports, untitled()).await.len(), before);
}

// A stranger on Untitled and anyone on a made-up id get the same not-found, and no unit opens
// for either.
#[tokio::test]
async fn a_stranger_and_a_made_up_id_answer_the_same_not_found_with_no_unit() {
    let mut world = test_world::world().await;
    let log = record_units(&mut world.ports);
    let cases = [
        ("a stranger", test_world::stranger(), untitled()),
        ("a made-up id", bob(), test_world::absent_commission()),
    ];

    for (label, actor, commission) in cases {
        let set = set_by(
            &world.ports,
            &actor,
            commission,
            DirectionStatus::ChangesRequested,
        )
        .await;

        assert!(
            matches!(
                set,
                Err(CommissionError::CommonError(CommonError::NotFound(
                    NotFoundEntity::Commission
                )))
            ),
            "{label}: {set:?}"
        );
    }
    assert_eq!(log.events(), Vec::<&str>::new());
    assert_eq!(
        fields_of(&world.ports, untitled()).await.direction_status(),
        None
    );
    assert_eq!(
        fields_of(&world.ports, untitled()).await.lifecycle_step(),
        &LifecycleStep::Draft
    );
}
