use std::sync::Arc;

use adapter_mem::fixture::{bob, untitled};
use chrono::Utc;
use domain::elements::commission::{ChangelogEntry, ChangelogEntryKind};
use test_support::recording::{CallLog, RecordingDatabase};

use super::*;
use crate::{
    commission::{CommissionError, Commissions},
    common_error::{CommonError, NotFoundEntity},
    den::test_acts::{FieldCall, field_call},
    test_world,
};

/// `actor`'s move of `commission` to `to`, through the use case.
async fn move_by(
    ports: &Ports,
    actor: &UserId,
    commission: CommissionId,
    to: LifecycleStep,
) -> CommissionResult<Output> {
    let command = Command {
        actor_id: actor.clone(),
        commission_id: commission,
        to,
    };
    Commissions::new(ports)
        .lifecycle()
        .move_to(command, Utc::now())
        .await
}

/// Every changelog entry of `commission`.
async fn entries(ports: &Ports, commission: CommissionId) -> Vec<ChangelogEntry> {
    ports
        .changelog
        .entries(&commission)
        .await
        .expect("the changelog reads")
}

/// The lifecycle step `commission` holds.
async fn step_of(ports: &Ports, commission: CommissionId) -> LifecycleStep {
    let stored = ports
        .commissions
        .find(&commission)
        .await
        .expect("the store reads")
        .expect("the commission is stored");
    stored.lifecycle_step
}

/// Records `begin`, `commit` and `rollback` on `ports`' database from here on.
fn record_units(ports: &mut Ports) -> CallLog {
    let log = CallLog::default();
    let recording = RecordingDatabase::new(ports.database.clone(), log.clone());
    ports.database = Arc::new(recording);
    log
}

// Bob, a Participant, moves Untitled from Draft to Active: one `lifecycle_moved` entry by him,
// naming the step held and the step moved to.
#[tokio::test]
async fn a_participant_moves_draft_to_active() {
    let world = test_world::world().await;
    let before = entries(&world.ports, untitled()).await.len();

    move_by(&world.ports, &bob(), untitled(), LifecycleStep::Active)
        .await
        .expect("bob moves Untitled");

    let stream = entries(&world.ports, untitled()).await;
    let [entry] = &stream[before..] else {
        panic!("one entry: {} of them", stream.len() - before);
    };
    let payload = serde_json::json!({ "from": "draft", "to": "active" });
    assert_eq!(entry.kind, ChangelogEntryKind::LifecycleMoved);
    assert_eq!(entry.actor_id, Some(bob()));
    assert_eq!(entry.payload, payload);
    assert_eq!(entry.note, None);
    assert_eq!(
        step_of(&world.ports, untitled()).await,
        LifecycleStep::Active
    );
}

// A move back to Draft is refused as `lifecycle_move_refused`, and nothing lands.
#[tokio::test]
async fn a_move_back_to_draft_is_refused() {
    let world = test_world::world().await;
    move_by(&world.ports, &bob(), untitled(), LifecycleStep::Active)
        .await
        .expect("bob moves Untitled to Active");
    let before = entries(&world.ports, untitled()).await.len();

    let refused = move_by(&world.ports, &bob(), untitled(), LifecycleStep::Draft).await;

    assert!(
        matches!(refused, Err(CommissionError::LifecycleMoveRefused)),
        "{refused:?}"
    );
    assert_eq!(
        step_of(&world.ports, untitled()).await,
        LifecycleStep::Active
    );
    assert_eq!(entries(&world.ports, untitled()).await.len(), before);
}

// A Disputed commission's move is refused as `commission_disputed`, and nothing lands.
#[tokio::test]
async fn a_disputed_commission_s_move_is_refused() {
    let world = test_world::world().await;
    field_call(
        &world.ports,
        &bob(),
        untitled(),
        FieldCall::RaiseDispute,
        Utc::now(),
    )
    .await
    .expect("bob raises a dispute");
    let before = entries(&world.ports, untitled()).await.len();

    let refused = move_by(&world.ports, &bob(), untitled(), LifecycleStep::Active).await;

    assert!(
        matches!(refused, Err(CommissionError::CommissionDisputed)),
        "{refused:?}"
    );
    assert_eq!(
        step_of(&world.ports, untitled()).await,
        LifecycleStep::Disputed
    );
    assert_eq!(entries(&world.ports, untitled()).await.len(), before);
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
        let moved = move_by(&world.ports, &actor, commission, LifecycleStep::Active).await;

        assert!(
            matches!(
                moved,
                Err(CommissionError::CommonError(CommonError::NotFound(
                    NotFoundEntity::Commission
                )))
            ),
            "{label}: {moved:?}"
        );
    }
    assert_eq!(log.events(), Vec::<&str>::new());
    assert_eq!(
        step_of(&world.ports, untitled()).await,
        LifecycleStep::Draft
    );
}
