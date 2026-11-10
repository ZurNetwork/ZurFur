//! The deadline sweeper's driver half: the wall clock and leader lock. The
//! policy lives in [`application::commission::Commissions::sweep_deadlines`],
//! run here as one sweep per tick under a Postgres advisory lock
//! (single-writer across api instances).

use std::time::Duration;

use chrono::Utc;

use crate::AppState;

/// The advisory-lock key for the sweeper's single-writer election. Must stay
/// unique among every advisory lock the app takes.
const DEADLINE_SWEEP_LOCK_KEY: i64 = 0xDEAD_11FE;

/// The wall-clock sweeper loop `main` spawns; never returns. Ticks on `every`
/// (clamped to at least one second) and runs [`sweep_tick`] each time.
pub async fn run_deadline_sweeper(runtime: AppState, every: Duration) {
    let every = every.max(Duration::from_secs(1));
    let mut ticker = tokio::time::interval(every);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        sweep_tick(&runtime).await;
    }
}

/// One tick: a sweep pass when this instance wins the leader lock, its outcome
/// logged. A failing pass is logged by its error's fixed text alone and retried
/// next tick.
async fn sweep_tick(runtime: &AppState) {
    match sweep_pass_as_leader(runtime).await {
        // Another instance holds the lock this tick — skip (no double-fire).
        Ok(None) => {}
        Ok(Some(0)) => {}
        Ok(Some(marked)) => tracing::info!(marked, "deadline sweep marked commissions Late"),
        Err(error) => tracing::error!(%error, "deadline sweep failed; retrying next tick"),
    }
}

/// Runs one sweep pass only if this instance wins the advisory lock
/// (transaction-scoped, not session-scoped, so a crash can never strand it),
/// at `Utc::now()` — the only place the sweeper touches the wall clock.
/// Returns `None` when another instance holds the lock, else `Some(count)`
/// marked Late.
async fn sweep_pass_as_leader(runtime: &AppState) -> anyhow::Result<Option<usize>> {
    let mut guard = runtime.pool.begin().await?;
    let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_xact_lock($1)")
        .bind(DEADLINE_SWEEP_LOCK_KEY)
        .fetch_one(&mut *guard)
        .await?;
    if !acquired {
        // Not the leader this tick — leave the lock-holder to sweep; do nothing.
        return Ok(None);
    }
    let app = runtime.app();
    let swept = app.commissions().sweep_deadlines(Utc::now()).await?;
    // Release the advisory lock by ending the (write-free) guard transaction.
    guard.rollback().await?;
    Ok(Some(swept.marked_late))
}

#[cfg(test)]
mod tests;
