//! The commission word and time checks' migration: every test seeds made-up commissions with
//! raw SQL before the node table's migration and runs every migration before the checks one.
//! Checks that each constraint refuses what its rule refuses and keeps the wire's first and
//! last instants; that each `_known` constraint lists exactly its enum's words and the time
//! bounds are stored in UTC whatever the migrating session's zone; that the `DO` check and the
//! constraints refuse the same rows; that a row already breaking a rule stops the migration
//! naming its id alone, unrecorded and with no constraint added; and that readers and writers
//! wait while the checks land. Requires a container runtime socket (DOCKER_HOST honored).

use std::{collections::BTreeSet, time::Duration};

use adapter_pg::PgPool;
use domain::elements::{
    commission::{DeadlineStatus, DirectionStatus, LifecycleStep},
    level::Level,
    maturity::MaturityRating,
};
use sqlx::{migrate::MigrateError, postgres::PgPoolOptions};
use strum::VariantArray;
use uuid::Uuid;

use crate::migration_steps::{
    bare_pool, run_migrations_before, run_migrations_through, try_run_migrations_through,
};
use crate::node_backfill::{DEN_1_VERSION, seed_commission, seed_owner};

/// The checks' migration, as sqlx numbers it.
const CHECKS_VERSION: i64 = 20261107164601;

/// How every refusal the migration's `DO` check raises begins.
const REFUSAL_OPENING: &str = "the commission checks stop: commission ";

/// The title every seeded commission carries, which no refusal may name.
const CANARY_TITLE: &str = "A distinctive checked title";

/// Puts every commission back on words and times each rule keeps.
const RESTORE: &str = "UPDATE commission SET lifecycle = 'draft', direction_status = NULL, \
     maturity = NULL, graphic = NULL, created_at = now(), deadline = NULL, archived_at = NULL, \
     disputed_from = NULL, disputed_at = NULL";

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

/// How many times the lock test polls for the migration to wait.
const LOCK_WAIT_POLLS: u32 = 500;

/// How long the lock test sleeps between two polls.
const LOCK_WAIT_INTERVAL: Duration = Duration::from_millis(20);

/// One rule the migration adds, named for the column it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::VariantArray)]
enum Rule {
    Lifecycle,
    DirectionStatus,
    Maturity,
    CreatedAt,
    Deadline,
    ArchivedAt,
    DisputedAt,
}

impl Rule {
    /// The constraint that holds this rule.
    fn constraint(self) -> &'static str {
        match self {
            Self::Lifecycle => "commission_lifecycle_known",
            Self::DirectionStatus => "commission_direction_status_known",
            Self::Maturity => "commission_maturity_known",
            Self::CreatedAt => "commission_created_at_in_range",
            Self::Deadline => "commission_deadline_in_range",
            Self::ArchivedAt => "commission_archived_at_in_range",
            Self::DisputedAt => "commission_disputed_at_in_range",
        }
    }

    /// Sets this rule's column on commission `$1` to `$2`, touching only what another CHECK
    /// ties to it: a Disputed step carries its dispute, and a rating its graphic flag.
    fn plant_statement(self) -> &'static str {
        match self {
            Self::Lifecycle => {
                "UPDATE commission SET lifecycle = $2, \
                 disputed_from = CASE WHEN $2 = 'disputed' THEN 'active' END, \
                 disputed_at = CASE WHEN $2 = 'disputed' THEN now() END WHERE id = $1"
            }
            Self::DirectionStatus => "UPDATE commission SET direction_status = $2 WHERE id = $1",
            Self::Maturity => {
                "UPDATE commission SET maturity = $2, \
                 graphic = CASE WHEN $2 IS NULL THEN NULL ELSE false END WHERE id = $1"
            }
            Self::CreatedAt => {
                "UPDATE commission SET created_at = $2::text::timestamptz WHERE id = $1"
            }
            Self::Deadline => {
                "UPDATE commission SET deadline = $2::text::timestamptz WHERE id = $1"
            }
            Self::ArchivedAt => {
                "UPDATE commission SET archived_at = $2::text::timestamptz WHERE id = $1"
            }
            Self::DisputedAt => {
                "UPDATE commission SET lifecycle = 'disputed', disputed_from = 'active', \
                 disputed_at = $2::text::timestamptz WHERE id = $1"
            }
        }
    }

    /// Whether this rule holds one of the four times.
    fn is_a_time(self) -> bool {
        matches!(
            self,
            Self::CreatedAt | Self::Deadline | Self::ArchivedAt | Self::DisputedAt
        )
    }

    /// A value this rule refuses, for the seeded refusals.
    fn offending_value(self) -> &'static str {
        match self {
            Self::Lifecycle => "not-a-lifecycle",
            Self::DirectionStatus => "not-a-direction",
            Self::Maturity => "explicit",
            Self::CreatedAt => "-infinity",
            Self::Deadline => "infinity",
            Self::ArchivedAt => "10000-01-01 00:00:00+00",
            Self::DisputedAt => "0001-12-31 23:59:59.999999+00 BC",
        }
    }

    /// A refused value no refusal may echo: a distinctive word, or a time whose year is.
    fn canary(self) -> (&'static str, &'static str) {
        match self {
            Self::Lifecycle => ("canarylifecycleword", "canarylifecycleword"),
            Self::DirectionStatus => ("canarydirectionword", "canarydirectionword"),
            Self::Maturity => ("canaryratingword", "canaryratingword"),
            Self::CreatedAt => ("12345-06-07 08:09:10+00", "12345"),
            Self::Deadline => ("4321-03-04 05:06:07+00 BC", "4321"),
            Self::ArchivedAt => ("54321-01-02 03:04:05+00", "54321"),
            Self::DisputedAt => ("infinity", "infinity"),
        }
    }

    /// The values the agreement test plants under this rule: every known word or a boundary
    /// instant, near misses, and NULL where the column allows it.
    fn grid(self) -> Vec<Option<String>> {
        let known: Vec<String> = match self {
            Self::Lifecycle => LifecycleStep::VARIANTS
                .iter()
                .map(|step| <&'static str>::from(step).to_owned())
                .collect(),
            Self::DirectionStatus => DirectionStatus::VARIANTS
                .iter()
                .map(|status| <&'static str>::from(status).to_owned())
                .collect(),
            Self::Maturity => MaturityRating::VARIANTS
                .iter()
                .map(|rating| <&'static str>::from(rating).to_owned())
                .collect(),
            _ => [FIRST_INSTANT, LAST_INSTANT, "2026-11-07 12:00:00+00"]
                .map(str::to_owned)
                .to_vec(),
        };
        let near_misses: &[&str] = match self {
            Self::Lifecycle => &["Draft", "draft ", "", "not-a-lifecycle"],
            Self::DirectionStatus => &["WaitingForInput", "waiting", ""],
            Self::Maturity => &["Safe", "explicit", ""],
            _ => &TIMES_OUT_OF_RANGE,
        };
        let nullable = !matches!(self, Self::Lifecycle | Self::CreatedAt | Self::DisputedAt);
        let null = nullable.then_some(None);
        known
            .into_iter()
            .chain(near_misses.iter().map(|value| (*value).to_owned()))
            .map(Some)
            .chain(null)
            .collect()
    }
}

/// A fresh database migrated up to just before the checks, holding two seeded commissions in
/// id order.
async fn just_before_the_checks() -> (PgPool, [Uuid; 2], impl Sized) {
    let (pool, container) = bare_pool().await;
    run_migrations_before(&pool, DEN_1_VERSION).await;
    seed_owner(&pool).await;
    let commissions = seed_two(&pool).await;
    run_migrations_before(&pool, CHECKS_VERSION).await;
    (pool, commissions, container)
}

/// A fresh database migrated through the checks, holding one seeded commission.
async fn checked() -> (PgPool, Uuid, impl Sized) {
    let (pool, [commission, _], container) = just_before_the_checks().await;
    run_migrations_through(&pool, CHECKS_VERSION).await;
    (pool, commission, container)
}

/// Seeds two commissions and answers their ids, the lower first.
async fn seed_two(pool: &PgPool) -> [Uuid; 2] {
    let first = seed_commission(pool, CANARY_TITLE, "private").await;
    let second = seed_commission(pool, CANARY_TITLE, "private").await;
    let mut both = [first, second];
    both.sort();
    both
}

/// The checks' migration's SQL, as embedded.
fn checks_sql() -> sqlx::SqlStr {
    adapter_pg::migrator()
        .migrations
        .iter()
        .find(|migration| migration.version == CHECKS_VERSION)
        .map(|migration| migration.sql.clone())
        .expect("the checks' migration is embedded")
}

/// Sets `rule`'s column on `commission` to `value` through `executor`.
async fn plant<'e>(
    executor: impl sqlx::PgExecutor<'e>,
    rule: Rule,
    commission: Uuid,
    value: Option<&str>,
) -> Result<(), sqlx::Error> {
    let planted = sqlx::query(rule.plant_statement())
        .bind(commission)
        .bind(value)
        .execute(executor)
        .await?;
    assert_eq!(planted.rows_affected(), 1, "one commission is planted");
    Ok(())
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

/// Whether the ledger records the checks' migration.
async fn ledger_records_it(pool: &PgPool) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = $1)")
        .bind(CHECKS_VERSION)
        .fetch_one(pool)
        .await
        .expect("read the ledger")
}

/// The stored definition of `constraint` on `pool`'s session, if the constraint exists.
async fn constraint_definition(pool: &PgPool, constraint: &str) -> Option<String> {
    sqlx::query_scalar("SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE conname = $1")
        .bind(constraint)
        .fetch_optional(pool)
        .await
        .expect("read the catalog's constraints")
}

/// Every commission row's id, words and times as one text each, in id order.
async fn commission_rows(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT ROW(id, lifecycle, direction_status, maturity, graphic, created_at, deadline, \
         archived_at, disputed_from, disputed_at)::text FROM commission ORDER BY id",
    )
    .fetch_all(pool)
    .await
    .expect("read the commissions")
}

/// Asserts that writing `value` to `rule`'s column on a checked database is refused by the
/// rule's own constraint.
async fn assert_refused(pool: &PgPool, commission: Uuid, rule: Rule, value: &str) {
    let refused = plant(pool, rule, commission, Some(value))
        .await
        .expect_err("the constraint refuses it");
    assert_eq!(
        refusing_constraint(&refused).as_deref(),
        Some(rule.constraint()),
        "{rule:?} = {value:?}: {refused}"
    );
}

/// The words a stored `_known` constraint lists, in order.
fn listed_words(definition: &str) -> BTreeSet<String> {
    definition
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(str::to_owned)
        .collect()
}

/// Whether the `DO` check refuses `value` under `rule`: planted on `commission` and the
/// migration's SQL run, in one transaction that is rolled back. Fails the test when the run
/// stops on anything but the `DO` check's own refusal.
async fn the_do_check_refuses(
    pool: &PgPool,
    commission: Uuid,
    rule: Rule,
    value: Option<&str>,
) -> bool {
    let mut attempt = pool.begin().await.expect("begin the attempt");
    plant(&mut *attempt, rule, commission, value)
        .await
        .expect("plant before the checks exist");
    let ran = sqlx::raw_sql(checks_sql()).execute(&mut *attempt).await;
    attempt.rollback().await.expect("roll the attempt back");
    let Err(stopped) = ran else {
        return false;
    };
    let message = stopped
        .as_database_error()
        .map(|error| error.message().to_owned())
        .unwrap_or_default();
    assert!(
        message.starts_with(REFUSAL_OPENING),
        "{rule:?} = {value:?} stopped past the DO check: {stopped}"
    );
    true
}

/// Whether the constraints refuse `value` under `rule`: the migration's SQL run, then
/// `value` planted on `commission`, in one transaction that is rolled back.
async fn the_constraint_refuses(
    pool: &PgPool,
    commission: Uuid,
    rule: Rule,
    value: Option<&str>,
) -> bool {
    let mut attempt = pool.begin().await.expect("begin the attempt");
    sqlx::raw_sql(checks_sql())
        .execute(&mut *attempt)
        .await
        .expect("the checks land on clean rows");
    let planted = plant(&mut *attempt, rule, commission, value).await;
    attempt.rollback().await.expect("roll the attempt back");
    let Err(refused) = planted else {
        return false;
    };
    assert_eq!(
        refusing_constraint(&refused).as_deref(),
        Some(rule.constraint()),
        "{rule:?} = {value:?} is refused by its own constraint: {refused}"
    );
    true
}

/// For each rule in turn, plants `value(rule)` on both seeded commissions, runs the migration
/// and answers how it stopped, then puts the rows back. Each run takes a fresh pool: a stopped
/// run leaves the migrator's lock on the connection it ran on.
async fn stop_on_each_rule(value: fn(Rule) -> &'static str) -> Vec<(Rule, [Uuid; 2], String)> {
    let db = test_support::pg::bare_db().await;
    let mut pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    run_migrations_before(&pool, DEN_1_VERSION).await;
    seed_owner(&pool).await;
    let commissions = seed_two(&pool).await;
    run_migrations_before(&pool, CHECKS_VERSION).await;
    let before = commission_rows(&pool).await;
    let mut stops = Vec::new();
    for rule in Rule::VARIANTS.iter().copied() {
        for commission in commissions {
            plant(&pool, rule, commission, Some(value(rule)))
                .await
                .expect("plant before the checks exist");
        }

        let stopped = try_run_migrations_through(&pool, CHECKS_VERSION)
            .await
            .expect_err("the migration stops");

        assert!(
            !ledger_records_it(&pool).await,
            "{rule:?}: the ledger does not record the stopped migration"
        );
        for absent in Rule::VARIANTS {
            assert_eq!(
                constraint_definition(&pool, absent.constraint()).await,
                None,
                "{rule:?}: {} was not added",
                absent.constraint()
            );
        }
        stops.push((
            rule,
            commissions,
            format!("{} | {stopped} | {stopped:?}", refusal(&stopped)),
        ));
        sqlx::query(RESTORE)
            .execute(&pool)
            .await
            .expect("put the rows back");
        pool.close().await;
        pool = adapter_pg::connect(db.url())
            .await
            .expect("pool reconnects");
    }
    let restored = commission_rows(&pool).await;
    assert_eq!(restored.len(), before.len(), "the rows are all still there");
    run_migrations_through(&pool, CHECKS_VERSION).await;
    assert!(ledger_records_it(&pool).await, "the clean run is recorded");
    assert_eq!(
        commission_rows(&pool).await,
        restored,
        "the clean run changed no row"
    );
    stops
}

/// A pool on `url` whose every session runs in the zone `zone`, set as each connection opens:
/// sqlx's own startup sets UTC.
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

/// Whether some statement in this test's own database waits for an ACCESS EXCLUSIVE lock on
/// `commission`.
async fn a_lock_on_commission_waits(pool: &PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_locks l
                        WHERE NOT l.granted
                          AND l.database = (SELECT oid FROM pg_database
                                            WHERE datname = current_database())
                          AND l.relation = 'commission'::regclass
                          AND l.mode = 'AccessExclusiveLock')",
    )
    .fetch_one(pool)
    .await
    .expect("read pg_locks")
}

/// Polls until a statement waits for an ACCESS EXCLUSIVE lock on `commission`, and fails the
/// test if none does within `LOCK_WAIT_POLLS` polls (about ten seconds).
async fn wait_until_a_lock_waits(pool: &PgPool) {
    for _ in 0..LOCK_WAIT_POLLS {
        if a_lock_on_commission_waits(pool).await {
            return;
        }
        tokio::time::sleep(LOCK_WAIT_INTERVAL).await;
    }
    panic!(
        "the migration never waited for an ACCESS EXCLUSIVE lock on commission while the writer held one"
    );
}

/// Asserts that `statement` on `pool` waits for a lock on `commission`: with a short lock
/// timeout, it gives up for want of the lock.
async fn assert_waits_for_the_lock(pool: &PgPool, statement: &'static str, commission: Uuid) {
    let mut other = pool.begin().await.expect("begin on another connection");
    sqlx::query("SET LOCAL lock_timeout = '200ms'")
        .execute(&mut *other)
        .await
        .expect("set a short lock timeout");
    let waited = sqlx::query(statement)
        .bind(commission)
        .execute(&mut *other)
        .await
        .expect_err("it waits for the lock");
    other
        .rollback()
        .await
        .expect("roll the other connection back");
    let code = waited
        .as_database_error()
        .and_then(|error| error.code())
        .map(|code| code.into_owned());
    assert_eq!(code.as_deref(), Some("55P03"), "{statement}: {waited}");
}

// Row by row, a lifecycle outside its six words is refused, case and spaces included.
#[tokio::test]
async fn an_unknown_lifecycle_is_refused() {
    let (pool, commission, _container) = checked().await;

    for value in ["not-a-lifecycle", "Draft", "draft ", "", "archived"] {
        assert_refused(&pool, commission, Rule::Lifecycle, value).await;
    }
}

// A direction status outside its three words is refused; NULL, no status, is kept.
#[tokio::test]
async fn an_unknown_direction_status_is_refused() {
    let (pool, commission, _container) = checked().await;

    for value in [
        "not-a-direction",
        "WaitingForInput",
        "waiting_for_input ",
        "",
    ] {
        assert_refused(&pool, commission, Rule::DirectionStatus, value).await;
    }
    plant(&pool, Rule::DirectionStatus, commission, None)
        .await
        .expect("no direction status is kept");
}

// A rating outside its four words is refused, the superseded `explicit` included.
#[tokio::test]
async fn an_unknown_rating_is_refused() {
    let (pool, commission, _container) = checked().await;

    for value in ["explicit", "Safe", "general", ""] {
        assert_refused(&pool, commission, Rule::Maturity, value).await;
    }
}

// Each of the four times refuses both infinities, the year 10000 and the year 0 (1 BC).
#[tokio::test]
async fn each_time_out_of_range_is_refused() {
    let (pool, commission, _container) = checked().await;

    for rule in Rule::VARIANTS
        .iter()
        .copied()
        .filter(|rule| rule.is_a_time())
    {
        for value in TIMES_OUT_OF_RANGE {
            assert_refused(&pool, commission, rule, value).await;
        }
    }
}

// On the fully migrated schema, each `_known` constraint lists exactly its enum's stored words,
// and so do the node level's and the deadline status's; Late is derived when read and never
// stored.
#[tokio::test]
async fn each_check_lists_exactly_its_enum_s_words() {
    let (pool, _db) = test_support::pg::fresh_pool().await;
    let lifecycle: BTreeSet<String> = LifecycleStep::VARIANTS
        .iter()
        .map(|step| <&'static str>::from(step).to_owned())
        .collect();
    let direction: BTreeSet<String> = DirectionStatus::VARIANTS
        .iter()
        .map(|status| <&'static str>::from(status).to_owned())
        .collect();
    let rating: BTreeSet<String> = MaturityRating::VARIANTS
        .iter()
        .map(|rating| <&'static str>::from(rating).to_owned())
        .collect();
    let level: BTreeSet<String> = Level::VARIANTS
        .iter()
        .map(|level| <&'static str>::from(level).to_owned())
        .collect();
    let stored_deadline_status: BTreeSet<String> = DeadlineStatus::VARIANTS
        .iter()
        .filter(|status| **status != DeadlineStatus::Late)
        .map(|status| <&'static str>::from(status).to_owned())
        .collect();
    let expected = [
        ("commission_lifecycle_known", lifecycle),
        ("commission_direction_status_known", direction),
        ("commission_maturity_known", rating),
        ("node_level_known", level),
        (
            "commission_deadline_status_delayed_only",
            stored_deadline_status,
        ),
    ];

    for (constraint, words) in expected {
        let definition = constraint_definition(&pool, constraint)
            .await
            .unwrap_or_else(|| panic!("{constraint} exists"));
        assert_eq!(
            listed_words(&definition),
            words,
            "{constraint}: {definition}"
        );
    }
}

// Migrated in a zone fourteen hours east of UTC, each time's bounds are still the wire's
// first instant and the store's last, read back in UTC.
#[tokio::test]
async fn the_time_bounds_are_stored_in_utc() {
    let db = test_support::pg::bare_db().await;
    let far_east = pool_in_zone(db.url(), FAR_EAST_ZONE).await;
    let zone: String = sqlx::query_scalar("SHOW TimeZone")
        .fetch_one(&far_east)
        .await
        .expect("read the session's zone");
    assert_eq!(
        zone, FAR_EAST_ZONE,
        "the migration runs in the far-east zone"
    );

    run_migrations_through(&far_east, CHECKS_VERSION).await;

    let utc = pool_in_zone(db.url(), "UTC").await;
    let first = format!("'{FIRST_INSTANT}'::timestamp with time zone");
    let last = format!("'{LAST_INSTANT}'::timestamp with time zone");
    for rule in Rule::VARIANTS
        .iter()
        .copied()
        .filter(|rule| rule.is_a_time())
    {
        let definition = constraint_definition(&utc, rule.constraint())
            .await
            .unwrap_or_else(|| panic!("{} exists", rule.constraint()));
        assert!(
            definition.contains(&first) && definition.contains(&last),
            "{}: {definition}",
            rule.constraint()
        );
    }
}

// A grid of planted values, each run against the `DO` check alone and against the
// constraints alone: each is refused by both or by neither, and every rule has values of both
// kinds.
#[tokio::test]
async fn the_do_check_and_the_constraints_refuse_the_same_rows() {
    let (pool, [commission, _], _container) = just_before_the_checks().await;

    for rule in Rule::VARIANTS.iter().copied() {
        let mut outcomes = BTreeSet::new();
        for value in rule.grid() {
            let value = value.as_deref();
            let by_the_do_check = the_do_check_refuses(&pool, commission, rule, value).await;
            let by_the_constraint = the_constraint_refuses(&pool, commission, rule, value).await;
            assert_eq!(
                by_the_do_check, by_the_constraint,
                "{rule:?} = {value:?}: the DO check refuses it {by_the_do_check}, the constraint {by_the_constraint}"
            );
            outcomes.insert(by_the_do_check);
        }
        assert_eq!(
            outcomes,
            BTreeSet::from([false, true]),
            "{rule:?}: the grid holds both kept and refused values"
        );
    }
}

// Per rule, two rows planted before the migration stop it, naming the lower id alone; it
// stays unrecorded and adds no constraint. With the rows put back, a clean run changes none.
#[tokio::test]
async fn a_planted_row_stops_it_naming_the_id() {
    let stops = stop_on_each_rule(Rule::offending_value).await;

    for (rule, [first, second], said) in stops {
        let named = format!("{REFUSAL_OPENING}{first} ");
        assert!(said.contains(&named), "{rule:?} names {first}: {said}");
        assert!(
            !said.contains(&second.to_string()),
            "{rule:?} names only the first offender: {said}"
        );
    }
}

// Distinctive words and years planted under every rule never reach the refusal, nor does
// the title.
#[tokio::test]
async fn the_refusals_name_ids_only() {
    let stops = stop_on_each_rule(|rule| rule.canary().0).await;

    for (rule, commissions, said) in stops {
        let (_planted, echo) = rule.canary();
        let said = commissions
            .iter()
            .fold(said, |text, id| text.replace(&id.to_string(), "<id>"));
        assert!(!said.contains(echo), "{rule:?} names no value: {said}");
        assert!(
            !said.contains(CANARY_TITLE),
            "{rule:?} names no title: {said}"
        );
    }
}

// A writer plants an unknown lifecycle and holds its transaction open; the migration, started
// meanwhile, waits for its lock before the DO check reads; the writer commits; the migration
// then stops naming its id, not with PostgreSQL's own constraint text. Then, with the checks'
// transaction open, a reader and a writer on other connections each wait for it.
#[tokio::test]
async fn a_reader_and_a_writer_wait_while_the_checks_land() {
    let db = test_support::pg::bare_db().await;
    let pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    run_migrations_before(&pool, DEN_1_VERSION).await;
    seed_owner(&pool).await;
    let [commission, _] = seed_two(&pool).await;
    run_migrations_before(&pool, CHECKS_VERSION).await;
    let mut writer = pool.begin().await.expect("begin the writer");
    plant(
        &mut *writer,
        Rule::Lifecycle,
        commission,
        Some("not-a-lifecycle"),
    )
    .await
    .expect("plant an unknown lifecycle, uncommitted");
    let migration_pool = pool.clone();
    let migration =
        tokio::spawn(
            async move { try_run_migrations_through(&migration_pool, CHECKS_VERSION).await },
        );

    wait_until_a_lock_waits(&pool).await;
    writer.commit().await.expect("commit the unknown lifecycle");
    let stopped = migration
        .await
        .expect("join the migration")
        .expect_err("the migration stops");

    let said = refusal(&stopped);
    assert!(
        said.starts_with(&format!("{REFUSAL_OPENING}{commission} ")),
        "the DO check names the row committed while it waited: {said}"
    );
    sqlx::query(RESTORE)
        .execute(&pool)
        .await
        .expect("put the rows back");
    pool.close().await;
    let pool = adapter_pg::connect(db.url())
        .await
        .expect("pool reconnects");
    let mut checks = pool.begin().await.expect("begin the checks");
    sqlx::raw_sql(checks_sql())
        .execute(&mut *checks)
        .await
        .expect("the checks land on clean rows");
    assert_waits_for_the_lock(
        &pool,
        "SELECT count(*) FROM commission WHERE id <> $1",
        commission,
    )
    .await;
    assert_waits_for_the_lock(
        &pool,
        "UPDATE commission SET deadline = now() WHERE id = $1",
        commission,
    )
    .await;
    checks.rollback().await.expect("roll the checks back");
}
