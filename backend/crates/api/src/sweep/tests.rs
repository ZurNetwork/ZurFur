use std::sync::Arc;

use application::commission::create;
use chrono::Utc;
use domain::{
    elements::{commission::CommissionId, did::Did},
    ports::UnitOfWork,
};
use test_support::{
    log_capture::{self, CapturedLog},
    pg_app::pg_runtime,
    store_doubles::{ScriptedStanding, ScriptedStandingDatabase, StandingScript},
};

use super::{DEADLINE_SWEEP_LOCK_KEY, sweep_pass_as_leader, sweep_tick};
use crate::AppState;

/// The owner's DID, which no line may carry.
const OWNER: &str = "did:plc:sweep-log-owner";

/// The commission's title, which no line may carry.
const TITLE: &str = "Quiet sweep title";

// Finding 5: the sweep pass is single-writer. While another connection holds the
// sweeper's advisory lock, a pass must SKIP (`None`); once the lock frees, the pass
// runs (`Some`). Deterministic — `pg_try_advisory_xact_lock` is a non-blocking
// try-lock — and an empty DB means the pass that runs marks zero. This is the guard
// that stops two api instances both appending a Late entry for the same lapse.
#[tokio::test]
async fn a_pass_skips_while_another_instance_holds_the_sweeper_lock() {
    let pg = pg_runtime().await;
    let runtime = &pg.runtime;

    // Stand in for another instance mid-sweep: hold the xact-scoped lock open on a
    // separate connection (released only when this transaction ends).
    let mut holder = runtime.pool.begin().await.expect("begin holder txn");
    let held: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
        .bind(DEADLINE_SWEEP_LOCK_KEY)
        .fetch_one(&mut *holder)
        .await
        .expect("holder takes the lock");
    assert!(held, "the holder acquires the sweeper lock");

    // A pass now finds the lock taken and skips — no second writer.
    let skipped = sweep_pass_as_leader(runtime)
        .await
        .expect("pass runs without error");
    assert_eq!(skipped, None, "a pass skips while another holds the lock");

    // Release the lock; the next pass wins it and runs (empty DB → marks zero).
    holder.rollback().await.expect("release the lock");
    let ran = sweep_pass_as_leader(runtime)
        .await
        .expect("pass runs without error");
    assert_eq!(ran, Some(0), "once the lock frees, the pass runs");
}

/// A log capture at trace, and the guard that makes it this thread's subscriber.
fn capture_at_trace() -> (CapturedLog, tracing::subscriber::DefaultGuard) {
    let log = CapturedLog::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .with_writer(log.clone())
        .finish();
    let guard = log_capture::set_default(subscriber);
    (log, guard)
}

/// A store failure as PostgreSQL words one: its detail names the owner's DID and the title.
fn detailed_failure() -> anyhow::Error {
    let detail = format!("DETAIL: Key (owner_id)=({OWNER}) holds {TITLE:?}");
    anyhow::anyhow!(detail).context("the store refused the statement")
}

/// A commission of [`OWNER`]'s titled [`TITLE`], its deadline an hour gone, made through the
/// use case on `runtime`.
async fn lapsed_commission(runtime: &AppState) -> CommissionId {
    let did = Did::from(OWNER.to_owned());
    let owner = runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            let user = uow.users().provision(&did).await?;
            Ok(user.id)
        })
        .await
        .expect("the owner is recognized");
    let hour_ago = Utc::now() - chrono::Duration::hours(1);
    let command = create::Command {
        actor_id: owner,
        title: TITLE.parse().expect("a title"),
        maturity: None,
        deadline: Some(hour_ago),
    };
    let created = runtime
        .app()
        .commissions()
        .create(command, hour_ago - chrono::Duration::hours(1))
        .await
        .expect("the commission is made");
    created.id
}

// A tick whose system act fails logs that commission by its id, and a tick whose whole pass
// fails logs the driver's error line: neither line, nor any other at any level, carries the
// owner's DID, the commission's title or the store's detail.
#[tokio::test]
async fn a_failing_system_act_logs_no_did_name_or_detail() {
    let pg = pg_runtime().await;
    let commission = lapsed_commission(&pg.runtime).await;
    let script = StandingScript::default();
    let mut runtime = pg.runtime.clone();
    let pool = ScriptedStanding::new(runtime.commissions.clone(), script.clone());
    let units = ScriptedStandingDatabase::new(runtime.database.clone(), script.clone());
    runtime.commissions = Arc::new(pool);
    runtime.database = Arc::new(units);
    let (log, _guard) = capture_at_trace();

    script.fail_system_lock(commission, detailed_failure());
    sweep_tick(&runtime).await;
    script.fail_scan(detailed_failure());
    sweep_tick(&runtime).await;

    let text = log.text();
    let act_line = format!("commission={commission}");
    assert!(
        text.lines()
            .any(|line| line.contains(" WARN ") && line.contains(&act_line)),
        "the failing act is logged by its id: {text}"
    );
    assert!(
        text.lines().any(|line| line.contains(" ERROR ")
            && line.contains("deadline sweep failed; retrying next tick")),
        "the driver's error line fires: {text}"
    );
    for line in text.lines() {
        for leak in [OWNER, "did:plc:", TITLE, "DETAIL", "Key ("] {
            assert!(!line.contains(leak), "a line carries {leak}: {line}");
        }
    }
}
