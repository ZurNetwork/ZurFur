//! PostgreSQL adapter — Zurfur's app-private data boundary.
//!
//! This crate implements the domain port traits (see [`domain::ports`]) against
//! PostgreSQL: the store for app-owned rows keyed by UUIDv7, where the rows are
//! ours to shape and mutate transactionally. It is the private counterpart to
//! `adapter-atproto`, which owns the public boundary (user-owned records on a
//! PDS). See DESIGN/"Domains and Applications" for the split.
//!
//! Per the dependency rule, everything here depends on `domain` and never the
//! reverse; the `api` crate is the composition root that decides this adapter is
//! the one wired in. Each `Pg*` type holds a cloned [`PgPool`] (an `Arc` handle,
//! cheap to clone) and is constructed from one via `new`.
//!
//! # Layout
//!
//! - [`PgAccountRepo`] — accounts and memberships ([`domain::ports::AccountRepo`]).
//! - [`PgUserRepo`] — recognized visitors ([`domain::ports::UserRepo`]).
//! - [`PgProfileCache`] — read-through profile cache ([`domain::ports::ProfileCache`]).
//! - [`PgSessionStore`] — durable tower-sessions backing store.
//!
//! Pool lifecycle and the migrations embedded from `migrations/` are owned by the
//! free functions below; the binary calls them at boot.

use std::time::Duration;

use sqlx::postgres::PgPoolOptions;

/// Re-export of sqlx's connection pool so the composition root and the adapter
/// types share one `PgPool` type without `api` depending on sqlx directly. Every
/// `Pg*` repo in this crate is built from one of these (see their `new`).
pub use sqlx::PgPool;

mod account;
mod profile;
mod session_store;
mod user;
pub use account::PgAccountRepo;
pub use profile::PgProfileCache;
pub use session_store::PgSessionStore;
pub use user::PgUserRepo;

/// Eagerly opens a connection pool, failing fast when the database is
/// unreachable.
///
/// Capped at 5 connections with a 5s acquire timeout. The eager connect is
/// deliberate: the binary should refuse to boot against an unreachable database
/// — [`migrate`] has to run before serving anyway — rather than surface the
/// failure on the first request.
///
/// # Caveats
///
/// - Returns `Err` if no connection can be established within the timeout; the
///   caller is expected to treat this as a fatal boot error.
/// - For an assembly path that must succeed without a live database (e.g. the
///   in-memory e2e tests), use [`lazy_pool`] instead.
///
/// # Example
///
/// ```ignore
/// let pool = adapter_pg::connect(&database_url).await?;
/// adapter_pg::migrate(&pool).await?;
/// ```
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}

/// Builds a pool without connecting — the first query is what opens a connection.
///
/// Lets an `AppState` be assembled for routes that never touch the database (the
/// in-memory e2e tests of the sign-in flow), with no container or live server.
/// The URL is only validated for shape here, not dialed.
///
/// # Caveats
///
/// - Succeeds even against an unreachable database; any connection failure is
///   deferred to the first query. Pair with [`connect`] when you want boot to
///   fail fast instead.
/// - Returns `Err` only when `database_url` cannot be parsed into connect
///   options.
pub fn lazy_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new().connect_lazy(database_url)
}

/// Runs all migrations embedded from this crate's `migrations/` directory.
///
/// Embedded at compile time via `sqlx::migrate!`, so the running binary carries
/// its schema and needs no migration files on disk. Called on every boot;
/// already-applied migrations are skipped, making it safe to run unconditionally.
///
/// # Caveats
///
/// - Returns `Err` if a migration fails or the recorded migration history has
///   diverged from the embedded set (e.g. a checksum mismatch).
/// - Requires a reachable database; on a [`lazy_pool`] this is the call that
///   actually opens the connection.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!().run(pool).await
}

/// Liveness probe: `true` when a trivial `SELECT 1` round-trips within 2s.
///
/// Backs `GET /health` (200 up / 503 down). A tokio timeout is the authoritative
/// bound rather than the pool's `acquire_timeout`: a dead *cached* connection
/// fails on I/O during the query, not on acquire, so the acquire timeout alone
/// would not catch it.
///
/// # Caveats
///
/// - Never returns `Err`; any failure (timeout, connection error, query error)
///   collapses to `false`.
/// - Borderline-slow databases can flap, since the 2s bound covers acquiring a
///   connection *and* executing the query.
pub async fn is_reachable(pool: &PgPool) -> bool {
    // The tokio timeout is the authoritative bound: a dead cached connection
    // fails on I/O, not on acquire, so acquire_timeout alone is not enough.
    tokio::time::timeout(
        Duration::from_secs(2),
        sqlx::query("SELECT 1").execute(pool),
    )
    .await
    .is_ok_and(|result| result.is_ok())
}
