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
mod tests;
