//! The deadline axis over PostgreSQL, against a throwaway container:
//! `set_deadline` / `set_deadline_status` live on the [`UnitOfWork`]'s
//! commissions view (so a deadline write and its changelog entry commit
//! together), and `lapsed_deadlines` is the sweeper's **transactional**
//! candidate scan — asked on the same open unit that then marks Late, so no
//! commission can slip between the scan and the mark (the
//! same same-transaction posture as `commission_has_facts`). Requires a container
//! runtime socket (DOCKER_HOST honored).

use crate::seed;
use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::{DateTime, Utc};
use domain::{
    elements::{
        commission::{
            Commission, CommissionId, CommissionState, DeadlineChange, DeadlineStatus,
            DelayedChange, LateFacts, LifecycleStep, NewChangelogEntry, clear_deadline,
            clear_delayed, owes_late, schedule_deadline, set_delayed,
        },
        did::Did,
        user::User,
    },
    ports::{CommissionStore, Database},
};

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// Recognize a visitor in its own committed unit of work (`commission.owner_id`
/// references `users(id)`).
async fn provision(pool: &PgPool, did: &str) -> User {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::from(did.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// Insert a commission (optionally staged into a lifecycle step) in its own
/// committed unit of work, returning it.
async fn seed(
    pool: &PgPool,
    owner: &User,
    title: &str,
    deadline: Option<DateTime<Utc>>,
    step: Option<LifecycleStep>,
) -> Commission {
    seed::seed_chosen(pool, &owner.id, title, deadline, step).await
}

fn ts(s: &str) -> DateTime<Utc> {
    s.parse().expect("valid timestamp")
}

/// The state `store` holds for `id`.
async fn state_of(store: &PgCommissionStore, id: &CommissionId) -> CommissionState {
    let stored = store.find(id).await.expect("find").expect("exists");
    CommissionState::from(&stored)
}

/// AC1 (store layer) — the deadline and the Delayed flag set, move and clear through the
/// unit of work by compare-and-set, and `find` reads both back through the domain gates; a
/// stale change matches no row, and a dropped unit keeps nothing.
#[tokio::test]
async fn deadline_and_status_round_trip_through_the_unit() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:deadline-owner").await;
    let commission = seed(&pool, &owner, "Ref", None, None).await;
    let id = commission.id;
    let db = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());
    let capability = seed::participant_on(id);

    // Set the deadline and flag Delayed in one unit.
    let deadline = ts("2027-03-01T00:00:00Z");
    let before_it = ts("2026-01-01T00:00:00Z");
    let undated = state_of(&store, &id).await;
    let scheduled = schedule_deadline(&undated, deadline)
        .expect("allowed")
        .expect("a real set");
    let set = DeadlineChange::from(scheduled);
    let mut uow = db.begin().await.expect("begin");
    let landed = uow
        .commissions()
        .set_deadline(&capability, &set)
        .await
        .expect("set deadline");
    uow.commit().await.expect("commit");
    assert!(landed);
    let dated = state_of(&store, &id).await;
    let raised = set_delayed(&dated, before_it)
        .expect("allowed")
        .expect("a real set");
    let flag = DelayedChange::from(raised);
    let mut uow = db.begin().await.expect("begin");
    let landed = uow
        .commissions()
        .set_deadline_status(&capability, &flag)
        .await
        .expect("set delayed");
    uow.commit().await.expect("commit");
    assert!(landed);

    let found = store.find(&id).await.expect("find").expect("exists");
    assert_eq!(found.deadline, Some(deadline));
    assert_eq!(found.delayed_flag, Some(DeadlineStatus::Delayed));

    // A stale set of either matches no row: the deadline from none, the flag from none.
    let mut uow = db.begin().await.expect("begin");
    let stale_deadline = uow
        .commissions()
        .set_deadline(&capability, &set)
        .await
        .expect("a stale set runs");
    let stale_flag = uow
        .commissions()
        .set_deadline_status(&capability, &flag)
        .await
        .expect("a stale flag runs");
    uow.commit().await.expect("commit");
    assert!(!stale_deadline);
    assert!(!stale_flag);

    // Late is derived, never persisted: a passed deadline reads Late on lookup,
    // superseding the stored Delayed (and the column's CHECK forbids a persisted
    // `late` outright).
    let flagged = state_of(&store, &id).await;
    let moved = schedule_deadline(&flagged, ts("2020-01-01T00:00:00Z"))
        .expect("allowed")
        .expect("a real move");
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .set_deadline(&capability, &DeadlineChange::from(moved))
        .await
        .expect("move deadline into the past");
    uow.commit().await.expect("commit");
    let found = store.find(&id).await.expect("find").expect("exists");
    assert_eq!(
        found.deadline_status_at(Utc::now()),
        Some(DeadlineStatus::Late),
        "a passed deadline derives Late, superseding the stored Delayed"
    );
    assert_eq!(
        found.delayed_flag,
        Some(DeadlineStatus::Delayed),
        "Late is never stored"
    );

    // Clear the deadline: the flag stays, dormant. Its clear compares against the stored flag,
    // though the act sees no status, and lands.
    let past = state_of(&store, &id).await;
    let unscheduled = clear_deadline(&past)
        .expect("allowed")
        .expect("a real clear");
    let mut uow = db.begin().await.expect("begin");
    let landed = uow
        .commissions()
        .set_deadline(&capability, &DeadlineChange::from(unscheduled))
        .await
        .expect("clear deadline");
    uow.commit().await.expect("commit");
    assert!(landed);
    let dormant = state_of(&store, &id).await;
    let lowered = clear_delayed(&dormant, Utc::now())
        .expect("allowed")
        .expect("a real clear");
    let lowered = DelayedChange::from(lowered);
    let mut uow = db.begin().await.expect("begin");
    let landed = uow
        .commissions()
        .set_deadline_status(&capability, &lowered)
        .await
        .expect("clear status");
    let elsewhere = uow
        .commissions()
        .set_deadline(
            &seed::participant_on(CommissionId::from(uuid::Uuid::now_v7())),
            &set,
        )
        .await
        .expect("an unknown commission runs");
    uow.commit().await.expect("commit");
    assert_eq!(lowered.before(), None, "the act saw no status");
    assert!(landed);
    assert!(!elsewhere, "an absent commission matches no row");
    let found = store.find(&id).await.expect("find").expect("exists");
    assert_eq!(found.deadline, None);
    assert_eq!(found.delayed_flag, None);

    // A dropped (uncommitted) unit discards its staged deadline write.
    {
        let mut uow = db.begin().await.expect("begin");
        uow.commissions()
            .set_deadline(&capability, &set)
            .await
            .expect("staged set");
    }
    let found = store.find(&id).await.expect("find").expect("exists");
    assert_eq!(found.deadline, None, "a dropped unit rolls the set back");
}

/// Ruling E12 (store layer) — `lapsed_deadlines(now)` returns exactly the
/// commissions the sweeper must mark: deadline strictly before `now`, not
/// already Late, lifecycle not terminal (Completed/Cancelled skipped; Disputed
/// is not terminal and IS returned); ordered by deadline; and it sees writes
/// staged on the SAME open unit (the no-TOCTOU posture).
#[tokio::test]
async fn lapsed_deadlines_scans_exactly_the_sweepable_set() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:sweeper-owner").await;
    let now = ts("2020-06-01T00:00:00Z");

    let missed_later = seed(
        &pool,
        &owner,
        "Missed B",
        Some(ts("2020-02-01T00:00:00Z")),
        None,
    )
    .await;
    let missed = seed(
        &pool,
        &owner,
        "Missed A",
        Some(ts("2020-01-01T00:00:00Z")),
        None,
    )
    .await;
    let delayed = seed(
        &pool,
        &owner,
        "Slipping",
        Some(ts("2020-03-01T00:00:00Z")),
        None,
    )
    .await;
    let already_late = seed(
        &pool,
        &owner,
        "Late",
        Some(ts("2020-01-01T00:00:00Z")),
        None,
    )
    .await;
    let _future = seed(
        &pool,
        &owner,
        "Future",
        Some(ts("2099-01-01T00:00:00Z")),
        None,
    )
    .await;
    let _no_deadline = seed(&pool, &owner, "No deadline", None, None).await;
    let _completed = seed(
        &pool,
        &owner,
        "Done",
        Some(ts("2020-01-01T00:00:00Z")),
        Some(LifecycleStep::Completed),
    )
    .await;
    let _cancelled = seed(
        &pool,
        &owner,
        "Dropped",
        Some(ts("2020-01-01T00:00:00Z")),
        Some(LifecycleStep::Cancelled),
    )
    .await;
    let disputed = seed(
        &pool,
        &owner,
        "Contested",
        Some(ts("2020-04-01T00:00:00Z")),
        Some(LifecycleStep::Disputed),
    )
    .await;

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    {
        let slipping = CommissionState::from(&delayed);
        let raised = set_delayed(&slipping, ts("2020-02-15T00:00:00Z"))
            .expect("allowed before the deadline")
            .expect("a real set");
        uow.commissions()
            .set_deadline_status(
                &seed::participant_on(delayed.id),
                &DelayedChange::from(raised),
            )
            .await
            .expect("flag delayed");
        // Late is deduped on the changelog (no persisted Late), staged on the
        // SAME unit: a commission already logged Late is skipped by the scan.
        let already_late_facts = LateFacts {
            state: CommissionState::from(&already_late),
            owed: true,
        };
        let mark = owes_late(&already_late_facts, now).expect("a late entry is owed");
        uow.changelog()
            .append(&NewChangelogEntry::late(already_late.id, &mark, now))
            .await
            .expect("log late");

        let lapsed = uow
            .commissions()
            .lapsed_deadlines(now)
            .await
            .expect("scan lapses");
        let ids: Vec<_> = lapsed.iter().map(|l| l.id).collect();
        assert_eq!(
            ids,
            vec![missed.id, missed_later.id, delayed.id, disputed.id],
            "exactly the sweepable set, ordered by deadline"
        );
        assert_eq!(lapsed[0].deadline, ts("2020-01-01T00:00:00Z"));
        assert_eq!(lapsed[0].status, None);
        assert_eq!(
            lapsed[2].status,
            Some(DeadlineStatus::Delayed),
            "the scan carries the standing flag so the Late entry can name it"
        );
    }
    uow.rollback().await.expect("rollback");
}
