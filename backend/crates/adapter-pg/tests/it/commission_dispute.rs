//! The commission dispute columns' migration: every test seeds made-up commissions with raw SQL
//! before the node table's migration, runs every migration before the dispute one (the fill
//! gives each its node), plants at most one change, then runs the dispute migration and nothing
//! after it. Checks that a commission already Disputed stops it, naming its id alone, the
//! migration unrecorded and both columns absent, also when the Disputed row is committed while
//! the migration waits for its lock; that a clean run passes and changes no row; and that its
//! CHECK ties the dispute to the Disputed step. Requires a container runtime socket
//! (DOCKER_HOST honored).

use std::time::Duration;

use adapter_pg::PgPool;
use sqlx::migrate::MigrateError;
use uuid::Uuid;

use crate::migration_steps::{
    bare_pool, run_migrations_before, run_migrations_through, try_run_migrations_through,
};
use crate::node_backfill::{DEN_1_VERSION, seed_commission, seed_owner};

/// The dispute columns' migration, as sqlx numbers it.
const DISPUTE_VERSION: i64 = 20261026075811;

/// The CHECK the migration adds.
const DISPUTE_CHECK: &str = "commission_dispute_matches_its_step";

/// The title every seeded commission carries, which no refusal may name.
const CANARY_TITLE: &str = "A distinctive disputed title";

/// The level tokens, none of which a refusal may name.
const LEVEL_TOKENS: [&str; 3] = ["private", "listed", "public"];

/// Puts commission `$1` at the lifecycle token `$2`, touching nothing else.
const SET_LIFECYCLE: &str = "UPDATE commission SET lifecycle = $2 WHERE id = $1";

/// How many times the lock test polls for the migration to wait.
const LOCK_WAIT_POLLS: u32 = 500;

/// How long the lock test sleeps between two polls.
const LOCK_WAIT_INTERVAL: Duration = Duration::from_millis(20);

/// A fresh database migrated up to just before the node table, with its owner.
async fn just_before_den_1() -> (PgPool, impl Sized) {
    let (pool, container) = bare_pool().await;
    run_migrations_before(&pool, DEN_1_VERSION).await;
    seed_owner(&pool).await;
    (pool, container)
}

/// Seeds one commission at the lifecycle token `step`, Private, and answers its id.
async fn seed_at(pool: &PgPool, step: &str) -> Uuid {
    let commission = seed_commission(pool, CANARY_TITLE, "private").await;
    set_lifecycle(pool, commission, step).await;
    commission
}

/// Puts `commission` at the lifecycle token `step`.
async fn set_lifecycle(pool: &PgPool, commission: Uuid, step: &str) {
    let updated = sqlx::query(SET_LIFECYCLE)
        .bind(commission)
        .bind(step)
        .execute(pool)
        .await
        .expect("set a commission's lifecycle");
    assert_eq!(updated.rows_affected(), 1, "one commission is updated");
}

/// Whether the ledger records the dispute migration.
async fn ledger_records_it(pool: &PgPool) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = $1)")
        .bind(DISPUTE_VERSION)
        .fetch_one(pool)
        .await
        .expect("read the ledger")
}

/// Whether `commission` has the column `column`.
async fn commission_has_column(pool: &PgPool, column: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns
                        WHERE table_name = 'commission' AND column_name = $1)",
    )
    .bind(column)
    .fetch_one(pool)
    .await
    .expect("read the catalog's columns")
}

/// Every commission row's id, lifecycle token and node, in id order.
async fn commission_rows(pool: &PgPool) -> Vec<(Uuid, String, Option<Uuid>)> {
    sqlx::query_as("SELECT id, lifecycle, node_id FROM commission ORDER BY id")
        .fetch_all(pool)
        .await
        .expect("read the commissions")
}

/// The database's own refusal text inside `stopped`.
fn refusal(stopped: &MigrateError) -> String {
    let database_error = match stopped {
        MigrateError::Execute(error) | MigrateError::ExecuteMigration(error, _) => {
            error.as_database_error()
        }
        _ => None,
    };
    database_error
        .map(|error| error.message().to_owned())
        .unwrap_or_else(|| panic!("the database refused the migration: {stopped:?}"))
}

/// Asserts the migration stopped naming `named` and nothing of its row but its id: no title,
/// no level token; and that it left no ledger row and neither column.
async fn assert_stopped_naming(pool: &PgPool, stopped: &MigrateError, named: Uuid) {
    let message = refusal(stopped);
    assert!(
        message.contains(&named.to_string()),
        "the refusal names {named}: {message}"
    );
    let everything_it_says = format!("{stopped} {stopped:?}");
    assert!(
        !everything_it_says.contains(CANARY_TITLE),
        "the refusal names no title: {everything_it_says}"
    );
    for token in LEVEL_TOKENS {
        assert!(
            !everything_it_says.contains(token),
            "the refusal names no level token: {everything_it_says}"
        );
    }
    assert!(
        !ledger_records_it(pool).await,
        "the ledger does not record the stopped migration"
    );
    for column in ["disputed_from", "disputed_at"] {
        assert!(
            !commission_has_column(pool, column).await,
            "{column} was not added"
        );
    }
}

/// Whether some statement in this test's own database waits for a lock on `commission`.
async fn a_lock_on_commission_waits(pool: &PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_locks l
                        WHERE NOT l.granted
                          AND l.database = (SELECT oid FROM pg_database
                                            WHERE datname = current_database())
                          AND l.relation = 'commission'::regclass)",
    )
    .fetch_one(pool)
    .await
    .expect("read pg_locks")
}

/// Polls until a statement waits for a lock on `commission`, and fails the test if none does
/// within `LOCK_WAIT_POLLS` polls (about ten seconds).
async fn wait_until_a_lock_waits(pool: &PgPool) {
    for _ in 0..LOCK_WAIT_POLLS {
        if a_lock_on_commission_waits(pool).await {
            return;
        }
        tokio::time::sleep(LOCK_WAIT_INTERVAL).await;
    }
    panic!("the migration never waited for a lock on commission while the writer held one");
}

/// `statement` run on `pool` with `commission`, refused by the dispute CHECK.
async fn assert_refused_by_the_check(pool: &PgPool, commission: Uuid, statement: &'static str) {
    let refused = sqlx::query(statement)
        .bind(commission)
        .execute(pool)
        .await
        .expect_err("the CHECK refuses it");
    let constraint = refused
        .as_database_error()
        .and_then(|error| error.constraint())
        .map(str::to_owned);
    assert_eq!(constraint.as_deref(), Some(DISPUTE_CHECK), "{statement}");
}

// A commission already Disputed before the columns exist has no step to return to: the
// migration stops naming its id alone, unrecorded, both columns absent.
#[tokio::test]
async fn a_disputed_row_without_its_step_stops_it_naming_the_id() {
    let (pool, _container) = just_before_den_1().await;
    let disputed = seed_at(&pool, "active").await;
    run_migrations_before(&pool, DISPUTE_VERSION).await;
    set_lifecycle(&pool, disputed, "disputed").await;

    let stopped = try_run_migrations_through(&pool, DISPUTE_VERSION)
        .await
        .expect_err("the migration stops");

    assert_stopped_naming(&pool, &stopped, disputed).await;
}

// The CHECK holds the dispute to the step: the columns set only while Disputed, the time with
// the step, the step left one of the other five; a whole dispute is kept, and so is its clear.
#[tokio::test]
async fn the_check_ties_the_step_to_disputed() {
    let (pool, _container) = just_before_den_1().await;
    let commission = seed_at(&pool, "active").await;
    run_migrations_through(&pool, DISPUTE_VERSION).await;

    let refused = [
        "UPDATE commission SET disputed_from = 'active', disputed_at = now() WHERE id = $1",
        "UPDATE commission SET lifecycle = 'disputed' WHERE id = $1",
        "UPDATE commission SET lifecycle = 'disputed', disputed_from = 'active' WHERE id = $1",
        "UPDATE commission SET lifecycle = 'disputed', disputed_at = now() WHERE id = $1",
        "UPDATE commission SET lifecycle = 'disputed', disputed_from = 'disputed', \
         disputed_at = now() WHERE id = $1",
        "UPDATE commission SET lifecycle = 'disputed', disputed_from = 'archived', \
         disputed_at = now() WHERE id = $1",
    ];
    for statement in refused {
        assert_refused_by_the_check(&pool, commission, statement).await;
    }
    let disputed = sqlx::query(
        "UPDATE commission SET lifecycle = 'disputed', disputed_from = 'active', \
         disputed_at = now() WHERE id = $1",
    )
    .bind(commission)
    .execute(&pool)
    .await
    .expect("a whole dispute is kept");
    assert_eq!(disputed.rows_affected(), 1);
    assert_refused_by_the_check(
        &pool,
        commission,
        "UPDATE commission SET lifecycle = 'active' WHERE id = $1",
    )
    .await;
    let cleared = sqlx::query(
        "UPDATE commission SET lifecycle = disputed_from, disputed_from = NULL, \
         disputed_at = NULL WHERE id = $1",
    )
    .bind(commission)
    .execute(&pool)
    .await
    .expect("a whole clear is kept");
    assert_eq!(cleared.rows_affected(), 1);
}

// A writer puts a commission into Disputed and holds its transaction open; the migration,
// started meanwhile, waits for its lock; the writer commits; the migration then sees the row
// and stops naming its id, not with PostgreSQL's own CHECK text.
#[tokio::test]
async fn a_dispute_committed_while_the_migration_waits_stops_it_naming_the_id() {
    let (pool, _container) = just_before_den_1().await;
    let disputed = seed_at(&pool, "batched").await;
    run_migrations_before(&pool, DISPUTE_VERSION).await;
    let mut writer = pool.begin().await.expect("begin the writer");
    let updated = sqlx::query(SET_LIFECYCLE)
        .bind(disputed)
        .bind("disputed")
        .execute(&mut *writer)
        .await
        .expect("put the commission into Disputed, uncommitted");
    assert_eq!(updated.rows_affected(), 1, "one commission is disputed");
    let migration_pool = pool.clone();
    let migration =
        tokio::spawn(
            async move { try_run_migrations_through(&migration_pool, DISPUTE_VERSION).await },
        );

    wait_until_a_lock_waits(&pool).await;
    writer.commit().await.expect("commit the dispute");
    let stopped = migration
        .await
        .expect("join the migration")
        .expect_err("the migration stops");

    assert_stopped_naming(&pool, &stopped, disputed).await;
}

// Through the real path: Draft, Active and Completed commissions seeded, a planted Disputed row
// stops the migration naming that id alone, unrecorded, no column added; with the plant taken
// back a clean run passes, changes no row and leaves no dispute, and the CHECK then refuses a
// hand-set step on an Active row. The clean run takes a fresh pool: a stopped run leaves the
// migrator's lock on the connection it ran on.
#[tokio::test]
async fn a_seeded_run_stops_on_a_planted_dispute_then_runs_clean() {
    let db = test_support::pg::bare_db().await;
    let pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    run_migrations_before(&pool, DEN_1_VERSION).await;
    seed_owner(&pool).await;
    let _draft = seed_at(&pool, "draft").await;
    let active = seed_at(&pool, "active").await;
    let _completed = seed_at(&pool, "completed").await;
    run_migrations_before(&pool, DISPUTE_VERSION).await;
    let before = commission_rows(&pool).await;
    set_lifecycle(&pool, active, "disputed").await;

    let stopped = try_run_migrations_through(&pool, DISPUTE_VERSION)
        .await
        .expect_err("the migration stops");

    assert_stopped_naming(&pool, &stopped, active).await;
    set_lifecycle(&pool, active, "active").await;
    pool.close().await;
    let pool = adapter_pg::connect(db.url())
        .await
        .expect("pool reconnects");
    run_migrations_through(&pool, DISPUTE_VERSION).await;
    assert!(ledger_records_it(&pool).await, "the clean run is recorded");
    assert_eq!(commission_rows(&pool).await, before, "no row changed");
    let disputes: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM commission WHERE disputed_from IS NOT NULL OR disputed_at IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .expect("count the disputes");
    assert_eq!(disputes, 0, "no commission carries a dispute");
    assert_refused_by_the_check(
        &pool,
        active,
        "UPDATE commission SET disputed_from = 'draft', disputed_at = now() WHERE id = $1",
    )
    .await;
}
