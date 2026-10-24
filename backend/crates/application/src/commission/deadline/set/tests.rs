use adapter_mem::fixture::{bob, untitled};
use chrono::Utc;
use domain::{
    datetime::DateTimeUtc,
    elements::commission::{ChangelogEntryKind, LifecycleStep},
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

// Bob, a Participant, sets Untitled's deadline, moves it later and pulls it in: a
// `deadline_set`, then a `deadline_extended`, then a `deadline_set`, each naming the deadline
// held and the one set.
#[tokio::test]
async fn a_participant_sets_extends_and_pulls_in_the_deadline() {
    let world = test_world::world().await;
    let before = entries_of(&world.ports, untitled()).await.len();
    let calls = [
        "2099-03-01T00:00:00Z",
        "2099-04-01T00:00:00Z",
        "2099-02-01T00:00:00Z",
    ];

    for deadline in calls {
        let call = FieldCall::SetDeadline(at(deadline));
        field_call(&world.ports, &bob(), untitled(), call, Utc::now())
            .await
            .expect("bob sets the deadline");
    }

    let stream = entries_of(&world.ports, untitled()).await;
    let recorded: Vec<(ChangelogEntryKind, serde_json::Value)> = stream[before..]
        .iter()
        .map(|entry| (entry.kind, entry.payload.clone()))
        .collect();
    let expected = [
        (
            ChangelogEntryKind::DeadlineSet,
            serde_json::json!({ "from": null, "to": "2099-03-01T00:00:00Z" }),
        ),
        (
            ChangelogEntryKind::DeadlineExtended,
            serde_json::json!({ "from": "2099-03-01T00:00:00Z", "to": "2099-04-01T00:00:00Z" }),
        ),
        (
            ChangelogEntryKind::DeadlineSet,
            serde_json::json!({ "from": "2099-04-01T00:00:00Z", "to": "2099-02-01T00:00:00Z" }),
        ),
    ];
    assert_eq!(recorded, expected);
    assert_eq!(
        fields_of(&world.ports, untitled()).await.deadline(),
        Some(at("2099-02-01T00:00:00Z"))
    );
}

// While Disputed the deadline is frozen: an extension is refused as `commission_disputed`, and
// nothing lands.
#[tokio::test]
async fn a_disputed_commission_s_deadline_is_frozen() {
    let world = test_world::world().await;
    let dated = FieldCall::SetDeadline(at("2099-03-01T00:00:00Z"));
    field_call(&world.ports, &bob(), untitled(), dated, Utc::now())
        .await
        .expect("bob sets the deadline");
    world
        .backend
        .seed_lifecycle_step(untitled(), LifecycleStep::Disputed);
    let before = entries_of(&world.ports, untitled()).await.len();

    let extended = FieldCall::SetDeadline(at("2099-04-01T00:00:00Z"));
    let refused = field_call(&world.ports, &bob(), untitled(), extended, Utc::now()).await;

    assert!(
        matches!(refused, Err(CommissionError::CommissionDisputed)),
        "{refused:?}"
    );
    assert_eq!(entries_of(&world.ports, untitled()).await.len(), before);
    assert_eq!(
        fields_of(&world.ports, untitled()).await.deadline(),
        Some(at("2099-03-01T00:00:00Z"))
    );
}

// A stranger on Untitled and anyone on a made-up id get the same not-found, and nothing lands.
#[tokio::test]
async fn a_stranger_and_a_made_up_id_answer_the_same_not_found() {
    let world = test_world::world().await;
    let call = FieldCall::SetDeadline(at("2099-03-01T00:00:00Z"));
    let cases = [
        ("a stranger", test_world::stranger(), untitled()),
        ("a made-up id", bob(), test_world::absent_commission()),
    ];

    for (label, actor, commission) in cases {
        let set = field_call(&world.ports, &actor, commission, call, Utc::now()).await;

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
    assert_eq!(fields_of(&world.ports, untitled()).await.deadline(), None);
}
