//! The Account time checks' migration: tests seed made-up Accounts with raw SQL on the
//! schema just before it. Checks that each of the three times refuses both infinities, the
//! year 10000 and the year 0 by its constraint's name and keeps the first and last instants;
//! that the bounds are stored in UTC whatever the migrating session's zone; and that a row
//! already out of range stops the migration naming the column and the count, never a DID,
//! unrecorded and with no constraint added, while a clean run changes no row. Requires a
//! container runtime socket (DOCKER_HOST honored).

use adapter_pg::PgPool;
use sqlx::{migrate::MigrateError, postgres::PgPoolOptions};

use crate::account_fact_migration::{did, seed_account};
use crate::migration_steps::{
    run_migrations_before, run_migrations_through, try_run_migrations_through,
};

/// The time checks' migration, as sqlx numbers it.
const TIMES_VERSION: i64 = 20261107174549;

/// The first instant the wire carries, as the bounds write it.
const FIRST_INSTANT: &str = "0001-01-01 00:00:00+00";

/// The last instant the store keeps, as the bounds write it.
const LAST_INSTANT: &str = "9999-12-31 23:59:59.999999+00";

/// The instants just outside the bounds and the two infinities, each refused on every time.
const TIMES_OUT_OF_RANGE: [&str; 4] = [
    "infinity",
    "-infinity",
    "10000-01-01 00:00:00+00",
    "0001-12-31 23:59:59.999999+00 BC",
];

/// The zone the bounds' time zone test migrates in: fourteen hours east of UTC.
const FAR_EAST_ZONE: &str = "Pacific/Kiritimati";

/// Puts every Account back on times each rule keeps.
const RESTORE: &str =
    "UPDATE accounts SET created_at = now(), updated_at = now(), deleted_at = NULL";

/// One of `accounts`' three times.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::VariantArray)]
enum Time {
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

impl Time {
    /// The column as the migration's refusal names it, with its article.
    fn named(self) -> &'static str {
        match self {
            Self::CreatedAt => "a created_at",
            Self::UpdatedAt => "an updated_at",
            Self::DeletedAt => "a deleted_at",
        }
    }

    /// The constraint that holds this time in range.
    fn constraint(self) -> &'static str {
        match self {
            Self::CreatedAt => "accounts_created_at_in_range",
            Self::UpdatedAt => "accounts_updated_at_in_range",
            Self::DeletedAt => "accounts_deleted_at_in_range",
        }
    }

    /// Sets this time on every Account whose id is in `$1` to `$2`.
    fn plant_statement(self) -> &'static str {
        match self {
            Self::CreatedAt => {
                "UPDATE accounts SET created_at = $2::text::timestamptz WHERE id = ANY($1)"
            }
            Self::UpdatedAt => {
                "UPDATE accounts SET updated_at = $2::text::timestamptz WHERE id = ANY($1)"
            }
            Self::DeletedAt => {
                "UPDATE accounts SET deleted_at = $2::text::timestamptz WHERE id = ANY($1)"
            }
        }
    }
}

/// Sets `time` on each of `accounts` to `value`.
async fn plant(
    pool: &PgPool,
    time: Time,
    accounts: &[String],
    value: Option<&str>,
) -> Result<(), sqlx::Error> {
    let planted = sqlx::query(time.plant_statement())
        .bind(accounts)
        .bind(value)
        .execute(pool)
        .await?;
    assert_eq!(
        planted.rows_affected(),
        accounts.len() as u64,
        "every Account is planted"
    );
    Ok(())
}

/// Seeds the Accounts named by `labels` and answers their DIDs.
async fn seed_accounts(pool: &PgPool, labels: &[&str]) -> Vec<String> {
    let mut accounts = Vec::new();
    for label in labels {
        let account = did(label);
        seed_account(pool, &account, label).await;
        accounts.push(account);
    }
    accounts
}

/// The constraint that refused `error`, if a constraint did.
fn refusing_constraint(error: &sqlx::Error) -> Option<String> {
    error
        .as_database_error()
        .and_then(|database_error| database_error.constraint())
        .map(str::to_owned)
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

/// Whether the ledger records the time checks' migration.
async fn ledger_records_it(pool: &PgPool) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = $1)")
        .bind(TIMES_VERSION)
        .fetch_one(pool)
        .await
        .expect("read the ledger")
}

/// The stored definition of `constraint`, if the constraint exists.
async fn constraint_definition(pool: &PgPool, constraint: &str) -> Option<String> {
    sqlx::query_scalar("SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE conname = $1")
        .bind(constraint)
        .fetch_optional(pool)
        .await
        .expect("read the catalog's constraints")
}

/// Every Account row's id and times as one text each, in id order.
async fn account_rows(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT ROW(id, created_at, updated_at, deleted_at)::text FROM accounts ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .expect("read the accounts")
}

/// A pool on `url` whose every session runs in the zone `zone`, set as each connection opens.
async fn pool_in_zone(url: &str, zone: &'static str) -> PgPool {
    PgPoolOptions::new()
        .max_connections(2)
        .after_connect(move |connection, _meta| {
            Box::pin(async move {
                sqlx::query("SELECT set_config('TimeZone', $1, false)")
                    .bind(zone)
                    .execute(connection)
                    .await
                    .map(drop)
            })
        })
        .connect(url)
        .await
        .expect("pool connects")
}

// On the migrated schema, each time refuses both infinities, the year 10000 and the year 0
// (1 BC) by its own constraint, and keeps the first and last instants; deleted_at keeps NULL.
#[tokio::test]
async fn each_accounts_time_out_of_range_is_refused() {
    let (pool, _db) = test_support::pg::fresh_pool().await;
    let accounts = seed_accounts(&pool, &["rangedaccount"]).await;

    for time in <Time as strum::VariantArray>::VARIANTS.iter().copied() {
        for value in TIMES_OUT_OF_RANGE {
            let refused = plant(&pool, time, &accounts, Some(value))
                .await
                .expect_err("the constraint refuses it");
            assert_eq!(
                refusing_constraint(&refused).as_deref(),
                Some(time.constraint()),
                "{time:?} = {value:?}: {refused}"
            );
        }
        for value in [FIRST_INSTANT, LAST_INSTANT] {
            plant(&pool, time, &accounts, Some(value))
                .await
                .unwrap_or_else(|error| panic!("{time:?} keeps {value}: {error}"));
        }
    }
    plant(&pool, Time::DeletedAt, &accounts, None)
        .await
        .expect("a live Account keeps no deletion time");
}

// Migrated in a zone fourteen hours east of UTC, each time's bounds are still the wire's
// first instant and the store's last, read back in UTC.
#[tokio::test]
async fn the_account_time_bounds_are_stored_in_utc() {
    let db = test_support::pg::bare_db().await;
    let far_east = pool_in_zone(db.url(), FAR_EAST_ZONE).await;

    run_migrations_through(&far_east, TIMES_VERSION).await;

    let utc = pool_in_zone(db.url(), "UTC").await;
    let first = format!("'{FIRST_INSTANT}'::timestamp with time zone");
    let last = format!("'{LAST_INSTANT}'::timestamp with time zone");
    for time in <Time as strum::VariantArray>::VARIANTS.iter().copied() {
        let definition = constraint_definition(&utc, time.constraint())
            .await
            .unwrap_or_else(|| panic!("{} exists", time.constraint()));
        assert!(
            definition.contains(&first) && definition.contains(&last),
            "{}: {definition}",
            time.constraint()
        );
    }
}

// Per time, two Accounts planted out of range before the migration stop it, naming the
// column and the count of two, and no DID; it stays unrecorded and adds no constraint. With
// the rows put back, a clean run is recorded and changes none.
#[tokio::test]
async fn the_range_migration_refuses_by_count_without_a_did() {
    let db = test_support::pg::bare_db().await;
    let mut pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    run_migrations_before(&pool, TIMES_VERSION).await;
    let accounts = seed_accounts(&pool, &["countedfirst", "countedsecond"]).await;
    let before = account_rows(&pool).await;

    for time in <Time as strum::VariantArray>::VARIANTS.iter().copied() {
        plant(&pool, time, &accounts, Some("infinity"))
            .await
            .expect("plant before the checks exist");

        let stopped = try_run_migrations_through(&pool, TIMES_VERSION)
            .await
            .expect_err("the migration stops");

        let said = format!("{} | {stopped} | {stopped:?}", refusal(&stopped));
        let counted = format!("stop: 2 accounts have {} outside", time.named());
        assert!(
            said.contains(&counted),
            "{time:?} names its column and count: {said}"
        );
        assert!(!said.contains("did:"), "{time:?} names no DID: {said}");
        assert!(
            !ledger_records_it(&pool).await,
            "{time:?}: the ledger does not record the stopped migration"
        );
        for absent in <Time as strum::VariantArray>::VARIANTS {
            assert_eq!(
                constraint_definition(&pool, absent.constraint()).await,
                None,
                "{time:?}: {} was not added",
                absent.constraint()
            );
        }
        sqlx::query(RESTORE)
            .execute(&pool)
            .await
            .expect("put the rows back");
        pool.close().await;
        pool = adapter_pg::connect(db.url())
            .await
            .expect("pool reconnects");
    }
    let restored = account_rows(&pool).await;
    assert_eq!(restored.len(), before.len(), "the rows are all still there");
    run_migrations_through(&pool, TIMES_VERSION).await;
    assert!(ledger_records_it(&pool).await, "the clean run is recorded");
    assert_eq!(
        account_rows(&pool).await,
        restored,
        "the clean run changed no row"
    );
}
