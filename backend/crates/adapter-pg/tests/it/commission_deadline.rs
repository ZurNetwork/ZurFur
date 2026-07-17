//! The deadline axis over PostgreSQL (ZMVP-86), against a throwaway container:
//! `set_deadline` / `set_deadline_status` live on the [`UnitOfWork`]'s
//! commissions view (so a deadline write and its changelog entry commit
//! together), and `lapsed_deadlines` is the sweeper's **transactional**
//! candidate scan — asked on the same open unit that then marks Late, so no
//! commission can slip between the scan and the mark (ruling E12; the same
//! same-transaction posture as `commission_has_facts`). Requires a container
//! runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::{DateTime, Utc};
use domain::{
    elements::{
        commission::{
            ChangelogEntryKind, Commission, CommissionTitle, DeadlineStatus, LifecycleStep,
            NewChangelogEntry,
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
        .provision(&Did::new(did.to_string()))
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
    let mut commission = Commission::create(
        title.parse::<CommissionTitle>().expect("valid title"),
        owner.id,
        Utc::now(),
        deadline,
    );
    if let Some(step) = step {
        commission.lifecycle_step = step;
    }
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .create(&commission)
        .await
        .expect("create commission");
    uow.commit().await.expect("commit");
    commission
}

fn ts(s: &str) -> DateTime<Utc> {
    s.parse().expect("valid timestamp")
}

/// AC1 (store layer) — the deadline and the deadline-axis status set, replace,
/// and clear through the unit of work, and `find` reads both back through the
/// domain gates.
#[tokio::test]
async fn deadline_and_status_round_trip_through_the_unit() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:deadline-owner").await;
    let commission = seed(&pool, &owner, "Ref", None, None).await;
    let id = commission.id;
    let db = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());

    // Set the deadline and flag Delayed in one unit.
    let deadline = ts("2027-03-01T00:00:00Z");
    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        commissions
            .set_deadline(id, Some(deadline))
            .await
            .expect("set deadline");
        commissions
            .set_deadline_status(id, Some(DeadlineStatus::Delayed))
            .await
            .expect("set delayed");
    }
    uow.commit().await.expect("commit");

    let found = store.find(id).await.expect("find").expect("exists");
    assert_eq!(found.deadline, Some(deadline));
    assert_eq!(found.deadline_status, Some(DeadlineStatus::Delayed));

    // Late is derived, never persisted: a passed deadline reads Late on lookup,
    // superseding the stored Delayed (and the column's CHECK forbids a persisted
    // `late` outright). Then clear everything; an absent commission is a no-op.
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .set_deadline(id, Some(ts("2020-01-01T00:00:00Z")))
        .await
        .expect("move deadline into the past");
    uow.commit().await.expect("commit");
    let found = store.find(id).await.expect("find").expect("exists");
    assert_eq!(
        found.deadline_status,
        Some(DeadlineStatus::Late),
        "a passed deadline derives Late, superseding the stored Delayed"
    );

    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        commissions
            .set_deadline(id, None)
            .await
            .expect("clear deadline");
        commissions
            .set_deadline_status(id, None)
            .await
            .expect("clear status");
        commissions
            .set_deadline(
                domain::elements::commission::CommissionId::new(uuid::Uuid::now_v7()),
                Some(deadline),
            )
            .await
            .expect("absent commission is a no-op");
    }
    uow.commit().await.expect("commit");
    let found = store.find(id).await.expect("find").expect("exists");
    assert_eq!(found.deadline, None);
    assert_eq!(found.deadline_status, None);

    // A dropped (uncommitted) unit discards its staged deadline write.
    {
        let mut uow = db.begin().await.expect("begin");
        uow.commissions()
            .set_deadline(id, Some(deadline))
            .await
            .expect("staged set");
    }
    let found = store.find(id).await.expect("find").expect("exists");
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
        uow.commissions()
            .set_deadline_status(delayed.id, Some(DeadlineStatus::Delayed))
            .await
            .expect("flag delayed");
        // Late is deduped on the changelog (no persisted Late), staged on the
        // SAME unit: a commission already logged Late is skipped by the scan.
        uow.changelog()
            .append(&NewChangelogEntry::system(
                already_late.id,
                ChangelogEntryKind::Late,
                serde_json::json!({}),
                now,
            ))
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
