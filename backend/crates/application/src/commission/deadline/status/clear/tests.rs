use adapter_mem::fixture::{bob, untitled};
use chrono::Utc;
use domain::{
    datetime::DateTimeUtc,
    elements::commission::{ChangelogEntryKind, DeadlineStatus, LifecycleStep},
};

use crate::{
    commission::CommissionError,
    den::test_acts::{FieldCall, entries_of, field_call, fields_of},
    test_world,
};

/// A deadline from test text.
fn at(text: &str) -> DateTimeUtc {
    text.parse().expect("a timestamp")
}

// While Disputed the Delayed flag may still be taken down: Bob lowers it on a Disputed
// Untitled with a deadline, and one `delayed` entry lands.
#[tokio::test]
async fn delayed_is_taken_down_while_disputed() {
    let world = test_world::world().await;
    let raise = [
        FieldCall::SetDeadline(at("2099-03-01T00:00:00Z")),
        FieldCall::SetDelayed,
    ];
    for call in raise {
        field_call(&world.ports, &bob(), untitled(), call, Utc::now())
            .await
            .expect("bob raises the flag");
    }
    world
        .backend
        .seed_lifecycle_step(untitled(), LifecycleStep::Disputed);
    let before = entries_of(&world.ports, untitled()).await.len();

    field_call(
        &world.ports,
        &bob(),
        untitled(),
        FieldCall::ClearDelayed,
        Utc::now(),
    )
    .await
    .expect("a Disputed commission's flag is taken down");

    let stream = entries_of(&world.ports, untitled()).await;
    let [entry] = &stream[before..] else {
        panic!("one entry: {} of them", stream.len() - before);
    };
    let payload = serde_json::json!({
        "from": "delayed",
        "to": null,
        "deadline": "2099-03-01T00:00:00Z",
    });
    assert_eq!(entry.kind, ChangelogEntryKind::Delayed);
    assert_eq!(entry.actor_id, Some(bob()));
    assert_eq!(entry.payload, payload);
    assert_eq!(
        fields_of(&world.ports, untitled()).await.delayed_flag(),
        None
    );
}

// A standing Late refuses the clear as `commission_late`, and nothing lands; the flag stays.
#[tokio::test]
async fn a_late_commission_s_flag_stays_up() {
    let world = test_world::world().await;
    let raise = [
        FieldCall::SetDeadline(at("2099-03-01T00:00:00Z")),
        FieldCall::SetDelayed,
        FieldCall::SetDeadline(at("2020-01-01T00:00:00Z")),
    ];
    for call in raise {
        field_call(&world.ports, &bob(), untitled(), call, Utc::now())
            .await
            .expect("bob raises the flag, then the deadline lapses");
    }
    let before = entries_of(&world.ports, untitled()).await.len();

    let refused = field_call(
        &world.ports,
        &bob(),
        untitled(),
        FieldCall::ClearDelayed,
        Utc::now(),
    )
    .await;

    assert!(
        matches!(refused, Err(CommissionError::CommissionLate)),
        "{refused:?}"
    );
    assert_eq!(entries_of(&world.ports, untitled()).await.len(), before);
    assert_eq!(
        fields_of(&world.ports, untitled()).await.delayed_flag(),
        Some(DeadlineStatus::Delayed)
    );
}
