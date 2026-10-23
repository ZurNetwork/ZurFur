//! The deadline sweep as **pure policy**: driven directly at a chosen `now`
//! over the shared in-memory runtime — no wall clock, no HTTP, no leader
//! lock. The timer and the advisory-lock leader election that wrap it stay
//! in `api` and are tested there. Each lapsed commission is marked in its own
//! unit, re-read under its lock: test-support's doubles make a change between
//! the pool scan and the units, or fail one commission's lock.

#![allow(clippy::disallowed_methods, reason = "test seeding")]

use std::sync::Arc;

use adapter_mem::CommissionSeed;
use application::commission::SweepResult;
use application::transaction;
use chrono::{DateTime, Utc};
use composition::Runtime;
use domain::elements::commission::{ChangelogEntry, CommissionId, CommissionTitle, LifecycleStep};
use domain::elements::{did::Did, user::UserId};
use domain::ports::{ChangelogStore, CommissionStore, UnitOfWork};
use test_support::runtime::MemRuntime;
use test_support::store_doubles::{ScriptedStanding, ScriptedStandingDatabase, StandingScript};

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

/// Flags the manual "slipping" status as `owner_did`, a Participant, sets it at `at`: through
/// the use case.
async fn flag(fixture: &MemRuntime, id: CommissionId, owner_did: &str, at: DateTime<Utc>) {
    let command = application::commission::deadline::status::set::Command {
        actor_id: UserId::from(Did::from(owner_did.to_owned())),
        commission_id: id,
    };
    fixture
        .runtime
        .app()
        .commissions()
        .deadline()
        .status()
        .set(command, at)
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

/// Runs one deterministic sweep at the injected instant through the use case — exactly what
/// the composition root's interval task does, minus the wall clock.
async fn sweep(runtime: &Runtime, now: DateTime<Utc>) -> SweepResult {
    runtime
        .app()
        .commissions()
        .sweep_deadlines(now)
        .await
        .expect("sweep runs")
}

/// Puts the fixture's pool and units behind `script`'s doubles, and answers the runtime they
/// wrap, which reads and writes the real stores: what a test changes between scan and units.
fn scripted(fixture: &mut MemRuntime, script: &StandingScript) -> Runtime {
    let real = fixture.runtime.clone();
    let pool = ScriptedStanding::new(real.commissions.clone(), script.clone());
    let units = ScriptedStandingDatabase::new(real.database.clone(), script.clone());
    fixture.runtime.commissions = Arc::new(pool);
    fixture.runtime.database = Arc::new(units);
    real
}

/// The owner `owner_did` moves `id`'s deadline to `deadline` through the use case.
async fn move_deadline(
    runtime: &Runtime,
    id: CommissionId,
    owner_did: &str,
    deadline: DateTime<Utc>,
) {
    let command = application::commission::deadline::set::Command {
        actor_id: UserId::from(Did::from(owner_did.to_owned())),
        commission_id: id,
        deadline,
    };
    runtime
        .app()
        .commissions()
        .deadline()
        .set(command, seeded_at())
        .await
        .expect("the owner moves the deadline");
}

/// The kinds of `entries`, as their stable tokens.
fn kinds(entries: &[ChangelogEntry]) -> Vec<&'static str> {
    entries.iter().map(|entry| entry.kind.into()).collect()
}

// AC2 + AC5 + AC4 — the sweep records every missed deadline in one pass as a
// SYSTEM entry (actor NULL) whose payload names the missed deadline; a future
// deadline is untouched; a second sweep is a no-op (no duplicate entry — Late
// is already the system's standing word).
#[tokio::test]
async fn the_sweep_records_every_missed_deadline_once() {
    let fixture = test_support::runtime::mem(&Did::from("did:plc:sweeper".to_string())).build();
    let runtime = &fixture.runtime;
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
        sweep(runtime, after_past()).await.marked_late,
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
        sweep(runtime, after_past()).await.marked_late,
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
    let id = seed(
        &fixture,
        "did:plc:slipping",
        "Ref",
        Some(past()),
        LifecycleStep::Draft,
    )
    .await;
    flag(&fixture, id, "did:plc:slipping", seeded_at()).await;

    assert_eq!(sweep(runtime, after_past()).await.marked_late, 1);
    let log = entries(&*runtime.changelog, id).await;
    let kinds: Vec<&'static str> = log.iter().map(|entry| entry.kind.into()).collect();
    assert_eq!(
        kinds,
        ["delayed", "late"],
        "the owner's flag, then the system Late entry"
    );
    assert_eq!(log[1].actor_id, None);
    assert_eq!(
        log[1].payload["from"], "delayed",
        "the upgrade records the standing flag it replaced"
    );
}

// Ruling E12 scope — the sweeper skips terminal lifecycles (Completed and
// Cancelled): a closed commission's missed deadline is history, not lateness.
#[tokio::test]
async fn the_sweeper_skips_terminal_lifecycles() {
    let fixture = test_support::runtime::mem(&Did::from("did:plc:sweeper".to_string())).build();
    let runtime = &fixture.runtime;
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
        sweep(runtime, after_past()).await.marked_late,
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
        sweep(runtime, after_past()).await.marked_late,
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

// Another sweep logs a commission's Late between this one's scan and its unit: the unit
// re-reads under its lock that nothing is owed, and skips it.
#[tokio::test]
async fn a_lapse_already_logged_since_the_scan_is_skipped() {
    let mut fixture = test_support::runtime::mem(&Did::from("did:plc:sweeper".to_string())).build();
    let owner = "did:plc:raced-owner";
    let raced = seed(
        &fixture,
        owner,
        "Raced",
        Some(past()),
        LifecycleStep::Active,
    )
    .await;
    let script = StandingScript::default();
    let real = scripted(&mut fixture, &script);
    let other_instance = real.clone();
    script.after_scan(Box::new(move || {
        Box::pin(async move {
            let marked = sweep(&other_instance, after_past()).await;
            assert_eq!(marked.marked_late, 1, "the other sweep logs it first");
        })
    }));

    let swept = sweep(&fixture.runtime, after_past()).await;

    assert_eq!(swept.marked_late, 0, "nothing was owed under the lock");
    let log = entries(&*real.changelog, raced).await;
    assert_eq!(
        kinds(&log),
        ["late"],
        "exactly one late entry, the other sweep's"
    );
}

// The owner moves the deadline into the future between the scan and the unit: the unit reads
// the moved deadline under its lock, owes nothing, and appends nothing.
#[tokio::test]
async fn a_deadline_moved_since_the_scan_is_skipped() {
    let mut fixture = test_support::runtime::mem(&Did::from("did:plc:sweeper".to_string())).build();
    let owner = "did:plc:moving-owner";
    let moved = seed(
        &fixture,
        owner,
        "Moved",
        Some(past()),
        LifecycleStep::Active,
    )
    .await;
    let script = StandingScript::default();
    let real = scripted(&mut fixture, &script);
    let mover = real.clone();
    let later: DateTime<Utc> = "2099-01-01T00:00:00Z".parse().expect("valid timestamp");
    script.after_scan(Box::new(move || {
        Box::pin(async move { move_deadline(&mover, moved, owner, later).await })
    }));

    let swept = sweep(&fixture.runtime, after_past()).await;

    assert_eq!(swept.marked_late, 0, "the moved deadline hasn't lapsed");
    let log = entries(&*real.changelog, moved).await;
    assert!(
        !kinds(&log).contains(&"late"),
        "no late entry: {:?}",
        kinds(&log)
    );
}

// One commission's unit fails: it is logged by its id and the pass goes on, so the next lapsed
// commission is still marked.
#[tokio::test]
async fn one_failing_commission_does_not_stop_the_pass() {
    let mut fixture = test_support::runtime::mem(&Did::from("did:plc:sweeper".to_string())).build();
    let owner = "did:plc:failing-owner";
    let failing = seed(
        &fixture,
        owner,
        "Failing",
        Some(past()),
        LifecycleStep::Active,
    )
    .await;
    let later_miss: DateTime<Utc> = "2020-02-01T00:00:00Z".parse().expect("valid timestamp");
    let marked = seed(
        &fixture,
        owner,
        "Marked",
        Some(later_miss),
        LifecycleStep::Active,
    )
    .await;
    let script = StandingScript::default();
    let real = scripted(&mut fixture, &script);
    script.fail_system_lock(failing, anyhow::anyhow!("the commission's lock failed"));

    let swept = sweep(&fixture.runtime, after_past()).await;

    assert_eq!(swept.marked_late, 1, "the pass went on past the failure");
    assert!(entries(&*real.changelog, failing).await.is_empty());
    let log = entries(&*real.changelog, marked).await;
    assert_eq!(kinds(&log), ["late"], "the next commission is marked");
}

// The owner moves a missed deadline to another past date between the scan and the unit: the
// entry names the deadline the unit read under its lock, not the one the scan saw.
#[tokio::test]
async fn the_late_entry_names_the_locked_deadline() {
    let mut fixture = test_support::runtime::mem(&Did::from("did:plc:sweeper".to_string())).build();
    let owner = "did:plc:locked-owner";
    let moved = seed(
        &fixture,
        owner,
        "Moved",
        Some(past()),
        LifecycleStep::Active,
    )
    .await;
    let script = StandingScript::default();
    let real = scripted(&mut fixture, &script);
    let mover = real.clone();
    let still_missed: DateTime<Utc> = "2020-03-01T00:00:00Z".parse().expect("valid timestamp");
    script.after_scan(Box::new(move || {
        Box::pin(async move { move_deadline(&mover, moved, owner, still_missed).await })
    }));

    let swept = sweep(&fixture.runtime, after_past()).await;

    assert_eq!(swept.marked_late, 1);
    let log = entries(&*real.changelog, moved).await;
    let late = log
        .iter()
        .find(|entry| <&'static str>::from(entry.kind) == "late")
        .expect("a late entry");
    assert_eq!(late.actor_id, None, "the system entry carries no actor");
    assert_eq!(
        late.payload["deadline"], "2020-03-01T00:00:00Z",
        "the locked deadline, not the scanned one"
    );
}
