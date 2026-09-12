//! The deadline sweeper's driver half: the wall clock and leader lock. The
//! policy lives in [`application::commission::sweep_deadlines`], run here as
//! one sweep per tick under a Postgres advisory lock (single-writer across
//! api instances).

use std::sync::Arc;
use std::time::Duration;

use adapter_pg::PgPool;
use application::commission::sweep_deadlines;
use chrono::Utc;
use domain::ports::Database;

/// The advisory-lock key for the sweeper's single-writer election. Must stay
/// unique among every advisory lock the app takes.
const DEADLINE_SWEEP_LOCK_KEY: i64 = 0xDEAD_11FE;

/// The wall-clock sweeper loop `main` spawns; never returns. Ticks on `every`
/// (clamped to at least one second) and, when it wins the leader lock, runs
/// [`sweep_deadlines`] at `Utc::now()` — the only place the sweeper touches
/// the wall clock. A failing sweep is logged and retried next tick.
pub async fn run_deadline_sweeper(database: Arc<dyn Database>, pool: PgPool, every: Duration) {
    let every = every.max(Duration::from_secs(1));
    let mut ticker = tokio::time::interval(every);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        match sweep_pass_as_leader(database.as_ref(), &pool).await {
            // Another instance holds the lock this tick — skip (no double-fire).
            Ok(None) => {}
            Ok(Some(0)) => {}
            Ok(Some(marked)) => tracing::info!(marked, "deadline sweep marked commissions Late"),
            Err(error) => tracing::error!(%error, "deadline sweep failed; retrying next tick"),
        }
    }
}

/// Runs one sweep pass only if this instance wins the advisory lock
/// (transaction-scoped, not session-scoped, so a crash can never strand it).
/// Returns `None` when another instance holds the lock, else `Some(count)`
/// marked Late.
async fn sweep_pass_as_leader(
    database: &dyn Database,
    pool: &PgPool,
) -> anyhow::Result<Option<usize>> {
    let mut guard = pool.begin().await?;
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
        .bind(DEADLINE_SWEEP_LOCK_KEY)
        .fetch_one(&mut *guard)
        .await?;
    if !acquired {
        // Not the leader this tick — leave the lock-holder to sweep; do nothing.
        return Ok(None);
    }
    let swept = sweep_deadlines(database, Utc::now()).await?;
    // Release the advisory lock by ending the (write-free) guard transaction.
    guard.rollback().await?;
    Ok(Some(swept.marked_late))
}

#[cfg(test)]
mod tests {
    use super::{DEADLINE_SWEEP_LOCK_KEY, sweep_pass_as_leader};

    // Finding 5: the sweep pass is single-writer. While another connection holds the
    // sweeper's advisory lock, a pass must SKIP (`None`); once the lock frees, the pass
    // runs (`Some`). Deterministic — `pg_try_advisory_xact_lock` is a non-blocking
    // try-lock — and an empty DB means the pass that runs marks zero. This is the guard
    // that stops two api instances both appending a Late entry for the same lapse.
    #[tokio::test]
    async fn a_pass_skips_while_another_instance_holds_the_sweeper_lock() {
        let db = test_support::pg::fresh_db().await;
        let pool = adapter_pg::connect(db.url()).await.expect("pool connects");
        let database = adapter_pg::PgDatabase::new(pool.clone());

        // Stand in for another instance mid-sweep: hold the xact-scoped lock open on a
        // separate connection (released only when this transaction ends).
        let mut holder = pool.begin().await.expect("begin holder txn");
        let held: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
            .bind(DEADLINE_SWEEP_LOCK_KEY)
            .fetch_one(&mut *holder)
            .await
            .expect("holder takes the lock");
        assert!(held, "the holder acquires the sweeper lock");

        // A pass now finds the lock taken and skips — no second writer.
        let skipped = sweep_pass_as_leader(&database, &pool)
            .await
            .expect("pass runs without error");
        assert_eq!(skipped, None, "a pass skips while another holds the lock");

        // Release the lock; the next pass wins it and runs (empty DB → marks zero).
        holder.rollback().await.expect("release the lock");
        let ran = sweep_pass_as_leader(&database, &pool)
            .await
            .expect("pass runs without error");
        assert_eq!(ran, Some(0), "once the lock frees, the pass runs");
    }
}
