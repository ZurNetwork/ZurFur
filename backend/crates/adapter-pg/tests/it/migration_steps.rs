//! Shared steps for tests that drive the migrator themselves: an empty
//! database, then every migration before a version, or through it, so a test
//! can seed the old schema in between. Requires a container runtime socket
//! (DOCKER_HOST honored).

use adapter_pg::PgPool;
use sqlx::migrate::{MigrateError, Migrator};

/// A fresh, empty private database with no migrations applied; the second
/// element keeps the shared container alive for the test's duration.
pub async fn bare_pool() -> (PgPool, impl Sized) {
    let db = test_support::pg::bare_db().await;
    let pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    (pool, db)
}

/// Run every embedded migration strictly before `version`.
pub async fn run_migrations_before(pool: &PgPool, version: i64) {
    let mut migrator = adapter_pg::migrator();
    assert_names_an_embedded_migration(&migrator, version);
    let subset: Vec<_> = migrator
        .migrations
        .iter()
        .filter(|migration| migration.version < version)
        .cloned()
        .collect();
    assert!(
        !subset.is_empty() && subset.len() < migrator.migrations.len(),
        "the version constant matches an embedded migration"
    );
    migrator.migrations = subset.into();
    migrator
        .run(pool)
        .await
        .expect("the migrations before the version run");
}

/// Run every embedded migration up to and including `version`, and none after
/// it, so a later migration's change never reads as one `version` made.
pub async fn run_migrations_through(pool: &PgPool, version: i64) {
    try_run_migrations_through(pool, version)
        .await
        .expect("the migrations through the version run");
}

/// [`run_migrations_through`], handing back the migrator's error instead of
/// failing the test, for a migration expected to stop.
pub async fn try_run_migrations_through(pool: &PgPool, version: i64) -> Result<(), MigrateError> {
    let mut migrator = adapter_pg::migrator();
    assert_names_an_embedded_migration(&migrator, version);
    let subset: Vec<_> = migrator
        .migrations
        .iter()
        .filter(|migration| migration.version <= version)
        .cloned()
        .collect();
    migrator.migrations = subset.into();
    migrator.run(pool).await
}

/// Fails the test unless `version` is one of the embedded migrations.
fn assert_names_an_embedded_migration(migrator: &Migrator, version: i64) {
    let names_one = migrator
        .migrations
        .iter()
        .any(|migration| migration.version == version);
    assert!(
        names_one,
        "the version constant {version} names an embedded migration"
    );
}

/// Every version the ledger records, in order.
async fn applied_versions(pool: &PgPool) -> Vec<i64> {
    sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
        .fetch_all(pool)
        .await
        .expect("read the ledger")
}

#[tokio::test]
async fn before_stops_short_of_the_version_and_through_stops_at_it() {
    let every_version: Vec<i64> = adapter_pg::migrator()
        .migrations
        .iter()
        .map(|migration| migration.version)
        .collect();
    let middle = every_version[every_version.len() / 2];
    let next = every_version[every_version.len() / 2 + 1];
    let (pool, _container) = bare_pool().await;

    run_migrations_before(&pool, middle).await;
    let after_before = applied_versions(&pool).await;
    let expected_before: Vec<i64> = every_version
        .iter()
        .copied()
        .filter(|version| *version < middle)
        .collect();
    assert_eq!(
        after_before, expected_before,
        "before runs every migration older than the version and nothing else"
    );

    run_migrations_through(&pool, middle).await;
    let after_through = applied_versions(&pool).await;
    let expected_through: Vec<i64> = every_version
        .iter()
        .copied()
        .filter(|version| *version <= middle)
        .collect();
    assert_eq!(
        after_through, expected_through,
        "through adds the version itself and nothing after it"
    );
    assert!(
        !after_through.contains(&next),
        "the migration after the version stays unapplied"
    );
}
