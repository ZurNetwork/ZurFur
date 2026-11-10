use adapter_mem::fixture::{bob, untitled};
use chrono::Utc;
use domain::{
    datetime::DateTimeUtc,
    elements::commission::{ChangelogEntryKind, DeadlineStatus},
};

use crate::{
    commission::CommissionError,
    common_error::{CommonError, NotFoundEntity},
    den::test_acts::{FieldCall, entries_of, field_call, fields_of},
    test_world,
};

/// A deadline from test text.
fn at(text: &str) -> DateTimeUtc {
    text.parse().expect("a timestamp")
}

// While Disputed the Delayed flag may still be raised: Bob raises it on a Disputed Untitled
// with a deadline, and one `delayed` entry lands.
#[tokio::test]
async fn delayed_is_raised_while_disputed() {
    let world = test_world::world().await;
    let dated = FieldCall::SetDeadline(at("2099-03-01T00:00:00Z"));
    field_call(&world.ports, &bob(), untitled(), dated, Utc::now())
        .await
        .expect("bob sets the deadline");
    field_call(
        &world.ports,
        &bob(),
        untitled(),
        FieldCall::RaiseDispute,
        Utc::now(),
    )
    .await
    .expect("bob raises a dispute");
    let before = entries_of(&world.ports, untitled()).await.len();

    field_call(
        &world.ports,
        &bob(),
        untitled(),
        FieldCall::SetDelayed,
        Utc::now(),
    )
    .await
    .expect("a Disputed commission's flag is raised");

    let stream = entries_of(&world.ports, untitled()).await;
    let [entry] = &stream[before..] else {
        panic!("one entry: {} of them", stream.len() - before);
    };
    let payload = serde_json::json!({
        "from": null,
        "to": "delayed",
        "deadline": "2099-03-01T00:00:00Z",
    });
    assert_eq!(entry.kind, ChangelogEntryKind::Delayed);
    assert_eq!(entry.actor_id, Some(bob()));
    assert_eq!(entry.payload, payload);
    assert_eq!(
        fields_of(&world.ports, untitled()).await.delayed_flag(),
        Some(DeadlineStatus::Delayed)
    );
}

// The flag stands against a deadline and never against Late: with none it is refused as
// `no_deadline`, and past one as `commission_late`; nothing lands either way.
#[tokio::test]
async fn delayed_needs_a_deadline_and_is_refused_while_late() {
    let world = test_world::world().await;
    let before = entries_of(&world.ports, untitled()).await.len();

    let undated = field_call(
        &world.ports,
        &bob(),
        untitled(),
        FieldCall::SetDelayed,
        Utc::now(),
    )
    .await;
    let lapsed = FieldCall::SetDeadline(at("2020-01-01T00:00:00Z"));
    field_call(&world.ports, &bob(), untitled(), lapsed, Utc::now())
        .await
        .expect("bob sets a lapsed deadline");
    let dated = entries_of(&world.ports, untitled()).await.len();
    let late = field_call(
        &world.ports,
        &bob(),
        untitled(),
        FieldCall::SetDelayed,
        Utc::now(),
    )
    .await;

    assert!(
        matches!(undated, Err(CommissionError::NoDeadline)),
        "{undated:?}"
    );
    assert!(
        matches!(late, Err(CommissionError::CommissionLate)),
        "{late:?}"
    );
    assert_eq!(dated, before + 1, "only the deadline's entry");
    assert_eq!(entries_of(&world.ports, untitled()).await.len(), dated);
    assert_eq!(
        fields_of(&world.ports, untitled()).await.delayed_flag(),
        None
    );
}

// A stranger on Untitled and anyone on a made-up id get the same not-found.
#[tokio::test]
async fn a_stranger_and_a_made_up_id_answer_the_same_not_found() {
    let world = test_world::world().await;
    let cases = [
        ("a stranger", test_world::stranger(), untitled()),
        ("a made-up id", bob(), test_world::absent_commission()),
    ];

    for (label, actor, commission) in cases {
        let set = field_call(
            &world.ports,
            &actor,
            commission,
            FieldCall::SetDelayed,
            Utc::now(),
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
}
