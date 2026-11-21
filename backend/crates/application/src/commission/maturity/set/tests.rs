use std::sync::Arc;

use adapter_mem::fixture::{alice, bob, untitled};
use chrono::Utc;
use domain::elements::{
    commission::{ChangelogEntryKind, CommissionId},
    maturity::{Maturity, MaturityRating},
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

/// A rating of `rating`, graphic or not.
fn rated(rating: MaturityRating, graphic: bool) -> Maturity {
    Maturity { rating, graphic }
}

/// `actor`'s rating of `commission` to `to`, through the use case.
async fn rate_by(
    ports: &Ports,
    actor: &UserId,
    commission: CommissionId,
    to: Maturity,
) -> CommissionResult<()> {
    field_call(
        ports,
        actor,
        commission,
        FieldCall::SetMaturity(to),
        Utc::now(),
    )
    .await
}

// Alice, the owner, rates Untitled and then re-rates it: one `maturity_rated` entry by her each
// time, naming the rating and the Graphic flag it now carries and nothing of the one held.
#[tokio::test]
async fn the_owner_rates_and_re_rates() {
    let world = test_world::world().await;
    let before = entries_of(&world.ports, untitled()).await.len();
    let suggestive = rated(MaturityRating::Suggestive, false);
    let adult = rated(MaturityRating::Adult, true);

    rate_by(&world.ports, &alice(), untitled(), suggestive)
        .await
        .expect("alice rates it");
    rate_by(&world.ports, &alice(), untitled(), adult)
        .await
        .expect("alice re-rates it");

    let stream = entries_of(&world.ports, untitled()).await;
    let [first, second] = &stream[before..] else {
        panic!("two entries: {} of them", stream.len() - before);
    };
    let first_payload = serde_json::json!({ "rating": "suggestive", "graphic": false });
    let second_payload = serde_json::json!({ "rating": "adult", "graphic": true });
    for (entry, payload) in [(first, first_payload), (second, second_payload)] {
        assert_eq!(entry.kind, ChangelogEntryKind::MaturityRated);
        assert_eq!(entry.actor_id, Some(alice()));
        assert_eq!(entry.payload, payload);
        assert_eq!(entry.note, None);
    }
    assert_eq!(
        fields_of(&world.ports, untitled()).await.maturity(),
        Some(adult)
    );
}

// Giving the rating held, rating and Graphic flag alike, writes nothing and records nothing.
#[tokio::test]
async fn giving_the_held_rating_writes_nothing() {
    let world = test_world::world().await;
    let nudity = rated(MaturityRating::Nudity, false);
    rate_by(&world.ports, &alice(), untitled(), nudity)
        .await
        .expect("alice rates it");
    let before = entries_of(&world.ports, untitled()).await.len();

    rate_by(&world.ports, &alice(), untitled(), nudity)
        .await
        .expect("a rating of the held rating lands");

    assert_eq!(entries_of(&world.ports, untitled()).await.len(), before);
}

// Bob, a Participant who isn't the owner, is refused before any unit opens, and nothing lands.
#[tokio::test]
async fn a_participant_s_rating_is_forbidden_with_no_unit() {
    let mut world = test_world::world().await;
    let log = record_units(&mut world.ports);
    let before = entries_of(&world.ports, untitled()).await.len();

    let refused = rate_by(
        &world.ports,
        &bob(),
        untitled(),
        rated(MaturityRating::Safe, false),
    )
    .await;

    assert!(
        matches!(refused, Err(CommissionError::InsufficientPermissions)),
        "{refused:?}"
    );
    assert_eq!(log.events(), Vec::<&str>::new());
    assert_eq!(fields_of(&world.ports, untitled()).await.maturity(), None);
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
        ("a made-up id", alice(), test_world::absent_commission()),
    ];

    for (label, actor, commission) in cases {
        let answer = rate_by(
            &world.ports,
            &actor,
            commission,
            rated(MaturityRating::Adult, false),
        )
        .await;

        assert!(
            matches!(
                answer,
                Err(CommissionError::CommonError(CommonError::NotFound(
                    NotFoundEntity::Commission
                )))
            ),
            "{label}: {answer:?}"
        );
    }
    assert_eq!(log.events(), Vec::<&str>::new());
    assert_eq!(fields_of(&world.ports, untitled()).await.maturity(), None);
}
