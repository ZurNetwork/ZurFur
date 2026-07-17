//! The **deadline sweeper** (ZMVP-86, conductor ruling E12): the one place the
//! system acts on a commission — and the api crate's first background task.
//!
//! The deadline axis is otherwise entirely Participant-moved (the manual
//! Delayed flag, the deadline itself); the sweeper's whole authority is: when a
//! commission's deadline has passed, set **Late** on the deadline axis and say
//! so in the changelog as a **system entry** (no actor). It is provably scoped
//! to exactly that — it calls
//! [`lapsed_deadlines`](domain::ports::CommissionWrites::lapsed_deadlines)
//! (which already excludes terminal lifecycles, already-Late commissions, and
//! anything without a deadline — AC4) and
//! [`set_deadline_status`](domain::ports::CommissionWrites::set_deadline_status)
//! (scoped to the one deadline-axis column); it holds no handle that could move
//! a Lifecycle or a direction status.
//!
//! Two layers, split for determinism:
//!
//! - [`sweep_deadlines`] — the pure policy: one sweep **as of an injected
//!   `now`** (never a wall clock — the `datetime` doctrine), in **one unit of
//!   work**: scan, mark Late, append each system entry, commit together. Tests
//!   drive this directly with a chosen instant.
//! - [`run_deadline_sweeper`] — the wall-clock loop `main` spawns: a tokio
//!   interval (from [`Config::deadline_sweep_interval_secs`](crate::Config))
//!   calling [`sweep_deadlines`] with `Utc::now()`, logging failures and
//!   sweeping again next tick (a failed sweep rolls back whole and is simply
//!   retried by time).
//!
//! Kept a policy over Postgres — no broker, no queue (the ticket's note:
//! Kafka is a post-MVP horizon).

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use domain::{
    datetime::DateTimeUtc,
    elements::commission::{ChangelogEntryKind, NewChangelogEntry},
    ports::{Database, UnitOfWork},
};
use serde_json::json;

use crate::transaction;

/// Run **one** deadline sweep as of `now` (injected, never read from a wall
/// clock here — deterministic by construction), returning how many commissions
/// were marked Late.
///
/// One unit of work per sweep (ruling E12): the candidate scan
/// ([`lapsed_deadlines`](domain::ports::CommissionWrites::lapsed_deadlines) —
/// deadline passed, not already Late, lifecycle not terminal), each Late mark,
/// and each matching **system** changelog entry (actor `NULL`, payload naming
/// the missed `deadline` and the standing flag — `delayed` or null — it
/// replaced) commit atomically or roll back together, so a marked commission
/// without its Late entry is unrepresentable (Changelog DD D4). A standing
/// manual Delayed upgrades to Late here (Engineer ruling 2026-07-05); a
/// commission already Late is never re-marked or re-logged — the *next* entry
/// for the same commission takes a fresh deadline miss (extend, then miss
/// again).
pub async fn sweep_deadlines(database: &dyn Database, now: DateTimeUtc) -> anyhow::Result<usize> {
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let lapsed = uow.commissions().lapsed_deadlines(now).await?;
        for lapse in &lapsed {
            // Log-only: `Late` is derived on lookup and never persisted
            // (Engineer ruling 2026-07-08). This pass just records the
            // transition once, so hooks/plugins have an event to consume.
            let entry = NewChangelogEntry::system(
                lapse.id,
                ChangelogEntryKind::Late,
                json!({
                    "deadline": lapse.deadline,
                    "from": lapse.status.map(|s| s.as_str()),
                }),
                now,
            );
            uow.changelog().append(&entry).await?;
        }
        Ok(lapsed.len())
    })
    .await
}

/// The wall-clock sweeper loop — what the composition root spawns
/// (`tokio::spawn(api::run_deadline_sweeper(database, every))` in `main`; the
/// api crate's first background task). Never returns.
///
/// Ticks on a tokio interval (`every`, clamped to at least one second so a
/// zero config can't busy-spin or panic the timer; missed ticks delay rather
/// than burst) and runs [`sweep_deadlines`] at `Utc::now()` — the **only**
/// place the sweeper touches the wall clock. A failing sweep is logged and
/// retried on the next tick: the sweep is one transaction, so a failure marks
/// nothing halfway.
pub async fn run_deadline_sweeper(database: Arc<dyn Database>, every: Duration) {
    let every = every.max(Duration::from_secs(1));
    let mut ticker = tokio::time::interval(every);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        ticker.tick().await;
        match sweep_deadlines(database.as_ref(), Utc::now()).await {
            Ok(0) => {}
            Ok(marked) => tracing::info!(marked, "deadline sweep marked commissions Late"),
            Err(error) => tracing::error!(%error, "deadline sweep failed; retrying next tick"),
        }
    }
}
