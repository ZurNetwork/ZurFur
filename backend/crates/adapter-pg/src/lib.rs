use std::time::Duration;

use sqlx::postgres::PgPoolOptions;

pub use sqlx::PgPool;

mod session_store;
mod user;
pub use session_store::PgSessionStore;
pub use user::PgUserRepo;

/// Eagerly connects so the binary fails fast at boot when the database is
/// unreachable — migrations must run before serving anyway.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
}

/// Builds a pool without connecting — the first query is what opens a connection.
/// Lets an `AppState` be assembled for routes that never touch the database (the
/// in-memory e2e tests of the sign-in flow), with no container or live server.
pub fn lazy_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new().connect_lazy(database_url)
}

/// Runs all migrations embedded from this crate's `migrations/` directory.
/// Called on every boot; already-applied migrations are skipped.
pub async fn migrate(pool: &PgPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!().run(pool).await
}

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
