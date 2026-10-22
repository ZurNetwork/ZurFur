//! The deadline sweep as **pure policy**: driven directly at a chosen `now`
//! over the shared in-memory runtime — no wall clock, no HTTP, no leader
//! lock. The timer and the advisory-lock leader election that wrap it stay
//! in `api` and are tested there.

#![allow(clippy::disallowed_methods, reason = "test seeding")]

use adapter_mem::CommissionSeed;
use application::commission::{SweepResult, sweep_deadlines};
use application::transaction;
use chrono::{DateTime, Utc};
use domain::elements::commission::{
    ChangelogEntry, CommissionId, CommissionTitle, DeadlineStatus, LifecycleStep,
};
use domain::elements::did::Did;
use domain::ports::{ChangelogStore, CommissionStore, Database, UnitOfWork};
use test_support::runtime::MemRuntime;

/// A deadline that is long past.
fn past() -> DateTime<Utc> {
    "2020-01-01T00:00:00Z".parse().expect("valid timestamp")
}

/// An instant comfortably after [`past`] (the sweeps' injected `now`).
fn after_past() -> DateTime<Utc> {
    "2020-06-01T00:00:00Z".parse().expect("valid timestamp")
}

/// When every seeded commission was created — before [`past`], so a seeded
/// deadline never precedes its own `created_at`.
fn seeded_at() -> DateTime<Utc> {
    "2019-12-01T00:00:00Z".parse().expect("valid timestamp")
}

/// Seeds a commission in `step` with `deadline`, owned by a directly-provisioned user and
/// bound to its node; a create is born Draft, so its step is written onto its row.
async fn seed(
    fixture: &MemRuntime,
    owner_did: &str,
    title: &str,
    deadline: Option<DateTime<Utc>>,
    step: LifecycleStep,
) -> CommissionId {
    let owner_did = Did::from(owner_did.to_string());
    let owner = transaction(
        &*fixture.runtime.database,
        async move |uow: &mut dyn UnitOfWork| {
            let owner = uow.users().provision(&owner_did).await?;
            Ok(owner.id)
        },
    )
    .await
    .expect("provision the owner");
    let title: CommissionTitle = title.parse().expect("valid title");
    let seed = CommissionSeed {
        created_at: seeded_at(),
        deadline,
        ..CommissionSeed::private(title, owner)
    };
    fixture
        .backend
        .seed_bound_commission(&seed)
        .await
        .expect("seed a commission");
    fixture.backend.seed_lifecycle_step(seed.id, step);
    seed.id
}

/// Flags the manual "slipping" status a Participant would set.
async fn flag(database: &dyn Database, id: CommissionId, status: DeadlineStatus) {
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        uow.commissions()
            .set_deadline_status(&id, Some(status))
            .await
    })
    .await
    .expect("flag the deadline axis");
}

/// The deadline status of `id` now (Late derived, else the stored flag), as its
/// stable wire token.
async fn stored_deadline_status(
    commissions: &dyn CommissionStore,
    id: CommissionId,
) -> Option<&'static str> {
    commissions
        .find(&id)
        .await
        .expect("find commission")
        .expect("commission exists")
        .deadline_status_at(Utc::now())
        .map(<&'static str>::from)
}

/// The commission's changelog entries.
async fn entries(changelog: &dyn ChangelogStore, id: CommissionId) -> Vec<ChangelogEntry> {
    changelog.entries(&id).await.expect("inspect entries")
}

/// Runs one deterministic sweep at the injected instant — exactly what the
/// composition root's interval task does, minus the wall clock.
async fn sweep(database: &dyn Database, now: DateTime<Utc>) -> SweepResult {
    sweep_deadlines(database, now).await.expect("sweep runs")
}

// AC2 + AC5 + AC4 — the sweep records every missed deadline in one pass as a
// SYSTEM entry (actor NULL) whose payload names the missed deadline; a future
// deadline is untouched; a second sweep is a no-op (no duplicate entry — Late
// is already the system's standing word).
#[tokio::test]
async fn the_sweep_records_every_missed_deadline_once() {
    let fixture = test_support::runtime::mem(&Did::from("did:plc:sweeper".to_string())).build();
    let runtime = &fixture.runtime;
    let database = &*runtime.database;
    let first = seed(
        &fixture,
        "did:plc:one",
        "One",
        Some(past()),
        LifecycleStep::Draft,
    )
    .await;
    let second = seed(
        &fixture,
        "did:plc:two",
        "Two",
        Some("2020-02-01T00:00:00Z".parse().expect("valid timestamp")),
        LifecycleStep::Draft,
    )
    .await;
    let unbothered = seed(
        &fixture,
        "did:plc:three",
        "Future",
        Some("2099-01-01T00:00:00Z".parse().expect("valid timestamp")),
        LifecycleStep::Draft,
    )
    .await;
    let deadlineless = seed(
        &fixture,
        "did:plc:four",
        "Open-ended",
        None,
        LifecycleStep::Draft,
    )
    .await;

    assert_eq!(
        sweep(database, after_past()).await.marked_late,
        2,
        "both missed deadlines are marked in one sweep"
    );
    assert_eq!(
        stored_deadline_status(&*runtime.commissions, first).await,
        Some("late")
    );
    assert_eq!(
        stored_deadline_status(&*runtime.commissions, second).await,
        Some("late")
    );
    assert_eq!(
        stored_deadline_status(&*runtime.commissions, unbothered).await,
        None,
        "a future deadline is not late"
    );
    assert_eq!(
        stored_deadline_status(&*runtime.commissions, deadlineless).await,
        None,
        "a commission with no deadline never carries a deadline-axis value"
    );

    let log = entries(&*runtime.changelog, first).await;
    assert_eq!(log.len(), 1, "the system Late entry");
    let late = &log[0];
    assert_eq!(<&'static str>::from(late.kind), "late");
    assert_eq!(late.actor_id, None, "the system entry carries no actor");
    assert_eq!(
        late.payload["deadline"], "2020-01-01T00:00:00Z",
        "the entry names the missed deadline (a sentence without joins)"
    );
    assert_eq!(
        late.payload["from"],
        serde_json::Value::Null,
        "nothing was standing to upgrade from"
    );

    // Idempotent: an already-Late commission is not re-marked or re-logged.
    assert_eq!(
        sweep(database, after_past()).await.marked_late,
        0,
        "nothing new to mark"
    );
    assert_eq!(
        entries(&*runtime.changelog, first).await.len(),
        1,
        "no duplicate entry"
    );
}

// AC2 — a standing (manual) Delayed upgrades to Late when the deadline passes;
// one cell, so Late REPLACES Delayed and the system entry records what it
// upgraded from.
#[tokio::test]
async fn a_standing_delayed_upgrades_to_late() {
    let fixture = test_support::runtime::mem(&Did::from("did:plc:sweeper".to_string())).build();
    let runtime = &fixture.runtime;
    let database = &*runtime.database;
    let id = seed(
        &fixture,
        "did:plc:slipping",
        "Ref",
        Some(past()),
        LifecycleStep::Draft,
    )
    .await;
    flag(database, id, DeadlineStatus::Delayed).await;

    assert_eq!(sweep(database, after_past()).await.marked_late, 1);
    let log = entries(&*runtime.changelog, id).await;
    assert_eq!(log.len(), 1, "the system Late entry");
    assert_eq!(<&'static str>::from(log[0].kind), "late");
    assert_eq!(log[0].actor_id, None);
    assert_eq!(
        log[0].payload["from"], "delayed",
        "the upgrade records the standing flag it replaced"
    );
}

// Ruling E12 scope — the sweeper skips terminal lifecycles (Completed and
// Cancelled): a closed commission's missed deadline is history, not lateness.
#[tokio::test]
async fn the_sweeper_skips_terminal_lifecycles() {
    let fixture = test_support::runtime::mem(&Did::from("did:plc:sweeper".to_string())).build();
    let runtime = &fixture.runtime;
    let database = &*runtime.database;
    let owner = "did:plc:lifecycle-owner";
    let completed = seed(
        &fixture,
        owner,
        "Staged",
        Some(past()),
        LifecycleStep::Completed,
    )
    .await;
    let cancelled = seed(
        &fixture,
        owner,
        "Staged",
        Some(past()),
        LifecycleStep::Cancelled,
    )
    .await;

    assert_eq!(
        sweep(database, after_past()).await.marked_late,
        0,
        "no closed commission is marked"
    );
    assert_eq!(
        stored_deadline_status(&*runtime.commissions, completed).await,
        None
    );
    assert_eq!(
        stored_deadline_status(&*runtime.commissions, cancelled).await,
        None
    );
    assert!(
        entries(&*runtime.changelog, completed).await.is_empty(),
        "nothing was appended to the closed commission's stream"
    );
    assert!(
        entries(&*runtime.changelog, cancelled).await.is_empty(),
        "nothing was appended to the cancelled commission's stream"
    );
}

// The Late rule skips a Disputed commission: it is not terminal, yet its missed
// deadline records no `late` entry, while a working one beside it is marked.
#[tokio::test]
async fn a_disputed_commission_is_not_swept() {
    let fixture = test_support::runtime::mem(&Did::from("did:plc:sweeper".to_string())).build();
    let runtime = &fixture.runtime;
    let database = &*runtime.database;
    let owner = "did:plc:dispute-owner";
    let disputed = seed(
        &fixture,
        owner,
        "Staged",
        Some(past()),
        LifecycleStep::Disputed,
    )
    .await;
    let active = seed(
        &fixture,
        owner,
        "Staged",
        Some(past()),
        LifecycleStep::Active,
    )
    .await;

    assert_eq!(
        sweep(database, after_past()).await.marked_late,
        1,
        "only the working commission is marked"
    );
    assert!(
        entries(&*runtime.changelog, disputed).await.is_empty(),
        "nothing was appended to the disputed commission's stream"
    );
    let marked = entries(&*runtime.changelog, active).await;
    assert_eq!(marked.len(), 1, "the working commission's Late entry");
    assert_eq!(<&'static str>::from(marked[0].kind), "late");
}
