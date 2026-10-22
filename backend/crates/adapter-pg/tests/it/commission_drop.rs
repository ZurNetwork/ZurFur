//! The commission column drop: every test seeds made-up rows with raw SQL on a
//! database migrated through the node table's migration (some before it), runs
//! the fill, plants at most one difference, then runs the drop and nothing after
//! it. Checks that a title or level that differs from the commission's node, or a
//! commission with no node, stops the drop (naming an id alone, the drop
//! unrecorded and both columns still there); that a title renamed before the fill
//! and a node renamed after it both pass; and what the drop leaves: no title or
//! visibility column, `node_id` NOT NULL, the fill's transitional check, trigger
//! and function gone, and each commission's name and level on its node. Requires a
//! container runtime socket (DOCKER_HOST honored).

use adapter_pg::PgPool;
use sqlx::migrate::MigrateError;
use uuid::Uuid;

use crate::commission_fill::FILL_VERSION;
use crate::migration_steps::{
    bare_pool, run_migrations_before, run_migrations_through, try_run_migrations_through,
};
use crate::node_backfill::{DEN_1_VERSION, NodeRow, archive, seed_commission, seed_owner};

/// The commission column drop's migration, as sqlx numbers it.
const DROP_VERSION: i64 = 20261022165210;

/// The three level tokens.
const LEVEL_TOKENS: [&str; 3] = ["private", "listed", "public"];

/// The title every planted row of the refusal-text test carries.
const CANARY_TITLE: &str = "A distinctive commission title";

/// The title the title plant of the refusal-text test sets apart from the node's name.
const CANARY_APART: &str = "A distinctive title set apart";

/// A fresh database migrated up to just before the node table, with its owner.
async fn just_before_den_1() -> (PgPool, impl Sized) {
    let (pool, container) = bare_pool().await;
    run_migrations_before(&pool, DEN_1_VERSION).await;
    seed_owner(&pool).await;
    (pool, container)
}

/// A fresh database migrated through the node table's migration, with its owner.
async fn through_den_1() -> (PgPool, impl Sized) {
    let (pool, container) = just_before_den_1().await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    (pool, container)
}

/// The node a commission row points at.
async fn commission_node(pool: &PgPool, commission: Uuid) -> NodeRow {
    sqlx::query_as("SELECT n.* FROM node n JOIN commission c ON c.node_id = n.id WHERE c.id = $1")
        .bind(commission)
        .fetch_one(pool)
        .await
        .expect("read a commission's node")
}

/// The node id a commission row points at, or `None`.
async fn node_id_of(pool: &PgPool, commission: Uuid) -> Option<Uuid> {
    sqlx::query_scalar("SELECT node_id FROM commission WHERE id = $1")
        .bind(commission)
        .fetch_one(pool)
        .await
        .expect("read a commission's node_id")
}

/// Whether the ledger records `version`.
async fn ledger_records(pool: &PgPool, version: i64) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = $1)")
        .bind(version)
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

/// Whether `commission.node_id` is NOT NULL.
async fn node_id_is_not_null(pool: &PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT attnotnull FROM pg_attribute
         WHERE attrelid = 'commission'::regclass AND attname = 'node_id'",
    )
    .fetch_one(pool)
    .await
    .expect("read node_id's attribute")
}

/// Whether `commission` carries the constraint `name`.
async fn commission_has_constraint(pool: &PgPool, name: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_constraint
                        WHERE conrelid = 'commission'::regclass AND conname = $1)",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("read the catalog's constraints")
}

/// Whether `node` carries the trigger `name`.
async fn node_has_trigger(pool: &PgPool, name: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_trigger
                        WHERE tgrelid = 'node'::regclass AND tgname = $1)",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("read the catalog's triggers")
}

/// Whether a function named `name` exists.
async fn function_exists(pool: &PgPool, name: &str) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_proc WHERE proname = $1)")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("read the catalog's functions")
}

/// Set `commission`'s own title, a test-only update after the fill; no trigger copies it.
async fn set_title_apart(pool: &PgPool, commission: Uuid, title: &str) {
    let updated = sqlx::query("UPDATE commission SET title = $2 WHERE id = $1")
        .bind(commission)
        .bind(title)
        .execute(pool)
        .await
        .expect("set the title apart from the node's name");
    assert_eq!(updated.rows_affected(), 1, "one commission row is updated");
}

/// Set the level of `commission`'s node to Listed, a test-only update after the fill; no
/// trigger copies a level.
async fn widen_node_to_listed(pool: &PgPool, commission: Uuid) {
    let updated = sqlx::query(
        "UPDATE node SET level = 'listed' FROM commission c WHERE c.id = $1 AND node.id = c.node_id",
    )
    .bind(commission)
    .execute(pool)
    .await
    .expect("widen the commission's node");
    assert_eq!(updated.rows_affected(), 1, "one node is updated");
}

/// Rename `commission`'s node, as the den's write gate does.
async fn rename_node(pool: &PgPool, commission: Uuid, name: &str) {
    let updated = sqlx::query(
        "UPDATE node SET name = $2 FROM commission c WHERE c.id = $1 AND node.id = c.node_id",
    )
    .bind(commission)
    .bind(name)
    .execute(pool)
    .await
    .expect("rename the commission's node");
    assert_eq!(updated.rows_affected(), 1, "one node is updated");
}

/// Insert a commission row with no node after the fill, in this test's own database only:
/// `commission_has_its_node` is dropped for the insert, then added back exactly as the fill
/// added it, so the schema is the fill's and only the row differs.
async fn plant_node_less(pool: &PgPool, title: &str) -> Uuid {
    sqlx::query("ALTER TABLE commission DROP CONSTRAINT commission_has_its_node")
        .execute(pool)
        .await
        .expect("drop commission_has_its_node in the test's database");
    let node_less = seed_commission(pool, title, "private").await;
    sqlx::query(
        "ALTER TABLE commission ADD CONSTRAINT commission_has_its_node \
         CHECK (node_id IS NOT NULL) NOT VALID",
    )
    .execute(pool)
    .await
    .expect("add commission_has_its_node back as the fill did");
    node_less
}

/// The database's own refusal text inside `stopped`.
fn refusal(stopped: &MigrateError) -> String {
    let (MigrateError::Execute(error) | MigrateError::ExecuteMigration(error, _)) = stopped else {
        panic!("the database refused the drop: {stopped:?}");
    };
    let database_error = error
        .as_database_error()
        .unwrap_or_else(|| panic!("the database refused the drop: {stopped:?}"));
    database_error.message().to_owned()
}

/// Asserts the drop stopped naming `named`, left no ledger row, and left both columns.
async fn assert_stopped_naming(pool: &PgPool, stopped: &MigrateError, named: Uuid) {
    let message = refusal(stopped);
    assert!(
        message.contains(&named.to_string()),
        "the refusal names {named}: {message}"
    );
    assert!(
        !ledger_records(pool, DROP_VERSION).await,
        "the ledger does not record the stopped drop"
    );
    assert!(
        commission_has_column(pool, "title").await,
        "the title column still exists"
    );
    assert!(
        commission_has_column(pool, "visibility").await,
        "the visibility column still exists"
    );
}

// ─── What passes ───────────────────────────────────────────────────────────

#[tokio::test]
async fn after_the_drop_the_columns_are_gone_and_node_id_is_not_null() {
    let (pool, _container) = just_before_den_1().await;
    let made_before = seed_commission(&pool, "Made before the node table", "private").await;
    let renamed = seed_commission(&pool, "The old title", "public").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    let made_in_the_window = seed_commission(&pool, "Made in the window", "private").await;
    let archived = seed_commission(&pool, "An archived commission", "public").await;
    archive(&pool, archived).await;
    let listed = seed_commission(&pool, "A listed commission", "listed").await;
    set_title_apart(&pool, renamed, "The new title").await;

    run_migrations_through(&pool, DROP_VERSION).await;

    assert!(
        ledger_records(&pool, DROP_VERSION).await,
        "the drop is recorded"
    );
    assert!(
        !commission_has_column(&pool, "title").await,
        "the title column is gone"
    );
    assert!(
        !commission_has_column(&pool, "visibility").await,
        "the visibility column is gone"
    );
    assert!(node_id_is_not_null(&pool).await, "node_id is NOT NULL");
    assert!(
        !commission_has_constraint(&pool, "commission_has_its_node").await,
        "the fill's NOT VALID check is gone"
    );
    assert!(
        !node_has_trigger(&pool, "commission_columns_follow_their_node").await,
        "the sync trigger is gone"
    );
    assert!(
        !function_exists(&pool, "commission_columns_follow_their_node").await,
        "the sync trigger's function is gone"
    );
    assert!(
        node_has_trigger(&pool, "node_has_its_thing").await,
        "node_has_its_thing still stands"
    );
    let seeded = [
        (made_before, "Made before the node table", "private"),
        (renamed, "The new title", "public"),
        (made_in_the_window, "Made in the window", "private"),
        (archived, "An archived commission", "public"),
        (listed, "A listed commission", "listed"),
    ];
    for (commission, name, level) in seeded {
        let node = commission_node(&pool, commission).await;
        assert_eq!(
            (node.name.as_str(), node.level.as_str()),
            (name, level),
            "{name} keeps its name and level on its node"
        );
    }
    rename_node(&pool, listed, "Renamed after the drop").await;
    assert_eq!(
        commission_node(&pool, listed).await.name,
        "Renamed after the drop",
        "a node rename after the drop succeeds"
    );
}

#[tokio::test]
async fn a_title_renamed_before_the_fill_passes_the_drop() {
    let (pool, _container) = just_before_den_1().await;
    let renamed = seed_commission(&pool, "The old title", "private").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    set_title_apart(&pool, renamed, "The new title").await;

    run_migrations_through(&pool, DROP_VERSION).await;

    assert_eq!(
        commission_node(&pool, renamed).await.name,
        "The new title",
        "the node keeps the title renamed before the fill"
    );
}

#[tokio::test]
async fn a_node_renamed_after_the_fill_passes_the_drop() {
    let (pool, _container) = through_den_1().await;
    let commission = seed_commission(&pool, "Before", "private").await;
    run_migrations_through(&pool, FILL_VERSION).await;
    rename_node(&pool, commission, "After").await;

    run_migrations_through(&pool, DROP_VERSION).await;

    assert_eq!(commission_node(&pool, commission).await.name, "After");
}

#[tokio::test]
async fn a_database_with_no_commissions_drops() {
    let (pool, _container) = bare_pool().await;

    run_migrations_through(&pool, DROP_VERSION).await;

    assert!(
        ledger_records(&pool, DROP_VERSION).await,
        "the drop is recorded"
    );
    assert!(!commission_has_column(&pool, "title").await);
    assert!(!commission_has_column(&pool, "visibility").await);
}

// ─── Its refusals ──────────────────────────────────────────────────────────

#[tokio::test]
async fn a_title_that_differs_from_its_node_stops_the_drop() {
    let (pool, _container) = through_den_1().await;
    seed_commission(&pool, "A kept title", "private").await;
    let differing = seed_commission(&pool, "A seeded title", "private").await;
    run_migrations_through(&pool, FILL_VERSION).await;
    set_title_apart(&pool, differing, "A title set apart").await;

    let stopped = try_run_migrations_through(&pool, DROP_VERSION)
        .await
        .expect_err("the drop stops");

    assert_stopped_naming(&pool, &stopped, differing).await;
}

#[tokio::test]
async fn a_level_that_differs_stops_the_drop() {
    let (pool, _container) = through_den_1().await;
    seed_commission(&pool, "A kept level", "private").await;
    let differing = seed_commission(&pool, "A widened commission", "private").await;
    run_migrations_through(&pool, FILL_VERSION).await;
    widen_node_to_listed(&pool, differing).await;

    let stopped = try_run_migrations_through(&pool, DROP_VERSION)
        .await
        .expect_err("the drop stops");

    assert_stopped_naming(&pool, &stopped, differing).await;
    assert_eq!(
        commission_node(&pool, differing).await.level,
        "listed",
        "no level is re-synced"
    );
}

#[tokio::test]
async fn a_commission_without_a_node_stops_the_drop() {
    let (pool, _container) = through_den_1().await;
    seed_commission(&pool, "A bound commission", "private").await;
    run_migrations_through(&pool, FILL_VERSION).await;
    let node_less = plant_node_less(&pool, "A node-less commission").await;

    let stopped = try_run_migrations_through(&pool, DROP_VERSION)
        .await
        .expect_err("the drop stops");

    assert_stopped_naming(&pool, &stopped, node_less).await;
    assert_eq!(
        node_id_of(&pool, node_less).await,
        None,
        "the planted row still has no node"
    );
}

#[tokio::test]
async fn the_refusal_names_the_id_only_and_title_still_exists() {
    let title_apart = {
        let (pool, container) = through_den_1().await;
        let commission = seed_commission(&pool, CANARY_TITLE, "private").await;
        run_migrations_through(&pool, FILL_VERSION).await;
        set_title_apart(&pool, commission, CANARY_APART).await;
        (pool, container)
    };
    let level_apart = {
        let (pool, container) = through_den_1().await;
        let commission = seed_commission(&pool, CANARY_TITLE, "private").await;
        run_migrations_through(&pool, FILL_VERSION).await;
        widen_node_to_listed(&pool, commission).await;
        (pool, container)
    };
    let node_less = {
        let (pool, container) = through_den_1().await;
        run_migrations_through(&pool, FILL_VERSION).await;
        plant_node_less(&pool, CANARY_TITLE).await;
        (pool, container)
    };

    for (label, (pool, _container)) in [
        ("a title that differs", title_apart),
        ("a level that differs", level_apart),
        ("a commission without a node", node_less),
    ] {
        let stopped = try_run_migrations_through(&pool, DROP_VERSION)
            .await
            .expect_err("the drop stops");
        let everything_it_says = format!("{stopped} {stopped:?}");
        for canary in [CANARY_TITLE, CANARY_APART] {
            assert!(
                !everything_it_says.contains(canary),
                "{label}: the refusal never echoes a title: {everything_it_says}"
            );
        }
        for token in LEVEL_TOKENS {
            assert!(
                !everything_it_says.contains(token),
                "{label}: the refusal never names a level token: {everything_it_says}"
            );
        }
        assert!(
            commission_has_column(&pool, "title").await,
            "{label}: the title column still exists"
        );
        assert!(
            !ledger_records(&pool, DROP_VERSION).await,
            "{label}: the ledger does not record the stopped drop"
        );
    }
}
