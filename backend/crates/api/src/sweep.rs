//! The **deadline sweeper**'s driver half (ZMVP-86, conductor ruling E12): the
//! wall clock and the leader lock, and nothing else.
//!
//! The policy — what a sweep *does* — moved down to
//! [`application::commission::sweep_deadlines`] (ZMVP-205): one sweep as of an
//! injected `now`, in one unit of work. What is left here is what only a driver
//! can own:
//!
//! - [`run_deadline_sweeper`] — the wall-clock loop `main` spawns: a tokio
//!   interval (from [`Config::deadline_sweep_interval_secs`](crate::Config))
//!   calling the use case with `Utc::now()`, logging failures and sweeping again
//!   next tick (a failed sweep rolls back whole and is simply retried by time).
//! - [`sweep_pass_as_leader`] — Postgres advisory-lock leader election, so the
//!   pass is single-writer across api instances.
//!
//! Kept a policy over Postgres — no broker, no queue (the ticket's note:
//! Kafka is a post-MVP horizon).

use std::sync::Arc;
use std::time::Duration;

use adapter_pg::PgPool;
use application::commission::sweep_deadlines;
use chrono::Utc;
use domain::ports::Database;

/// The Postgres advisory-lock key that gives the deadline sweeper **single-writer
/// leader election** across api instances (finding 5). Every session/transaction
/// advisory lock in a database shares one 64-bit keyspace, so this constant must stay
/// UNIQUE among every advisory lock the app takes — if another advisory lock is ever
/// added, give it a different key. The value is arbitrary-but-fixed (`0xDEAD_11FE`, a
/// "deadline" mnemonic).
const DEADLINE_SWEEP_LOCK_KEY: i64 = 0xDEAD_11FE;

/// The wall-clock sweeper loop — what the composition root spawns
/// (`tokio::spawn(api::run_deadline_sweeper(database, pool, every))` in `main`;
/// the api crate's first background task). Never returns.
///
/// Ticks on a tokio interval (`every`, clamped to at least one second so a
/// zero config can't busy-spin or panic the timer; missed ticks delay rather
/// than burst) and, when it wins the sweeper's advisory lock, runs
/// [`sweep_deadlines`] at `Utc::now()` — the **only** place the sweeper touches
/// the wall clock. The advisory lock ([`sweep_pass_as_leader`]) makes the pass
/// **single-writer** across api instances, so two instances can't both append a
/// Late entry for the same lapse (finding 5). A failing sweep is logged and
/// retried on the next tick: the sweep is one transaction, so a failure marks
/// nothing halfway.
pub async fn run_deadline_sweeper(database: Arc<dyn Database>, pool: PgPool, every: Duration) {
    let every = every.max(Duration::from_secs(1));
    let mut ticker = tokio::time::interval(every);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        match sweep_pass_as_leader(database.as_ref(), &pool).await {
            // Another instance held the lock this tick and is doing (or did) the
            // pass — skipping is exactly the guard's job (no double-fire).
            Ok(None) => {}
            Ok(Some(0)) => {}
            Ok(Some(marked)) => tracing::info!(marked, "deadline sweep marked commissions Late"),
            Err(error) => tracing::error!(%error, "deadline sweep failed; retrying next tick"),
        }
    }
}

/// Run one sweep pass **only if** this instance wins the sweeper's advisory lock —
/// serialising the pass across api instances so two of them can't both observe "no
/// Late entry yet" and both append one for the same lapse (finding 5, the
/// double-fire). Returns `None` when another instance holds the lock (this pass is
/// skipped), else `Some(count)` of commissions this instance marked Late.
///
/// The lock is **transaction-scoped** (`pg_try_advisory_xact_lock`): Postgres releases
/// it automatically when this guard transaction ends — including on an error return or
/// a panic that unwinds the task — so a crashed sweep can never strand it. A
/// *session*-scoped `pg_advisory_lock` would strand it here: returning the borrowed
/// connection to the pool does **not** end its session, so a lock left un-unlocked
/// (e.g. by a panic between lock and unlock) would silently freeze every future pass.
/// The empty guard transaction is held open across the sweep, which runs its own unit
/// of work on a separate pooled connection (so a pass briefly checks out two
/// connections — well within the pool).
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
