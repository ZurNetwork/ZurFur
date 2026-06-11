use std::time::Duration;

use sqlx::postgres::PgPoolOptions;

pub use sqlx::PgPool;

/// Eagerly connects so the binary fails fast at boot when the database is
/// unreachable — migrations must run before serving anyway.
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(5))
        .connect(database_url)
        .await
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
