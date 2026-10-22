//! The commission column drop: every test seeds made-up rows with raw SQL on a
//! database migrated through the node table's migration (some before it), runs
//! the fill, plants at most one difference, then runs the drop and nothing after
//! it. Checks that each of these stops the drop, naming an id alone, the drop
//! unrecorded and both columns still there: a title or level that differs from the
//! commission's node; a commission with no node of its own (none, a node deleted by
//! hand, or a node that isn't its own); and an edit committed while the drop waits
//! for its lock (a level edit on the node, or a title rename on the commission).
//! Checks that a title renamed before the fill and a node renamed after it both
//! pass; and what the drop leaves: no title or visibility column, `node_id` NOT
//! NULL, the fill's transitional check, trigger and function gone, and each
//! commission's name and level on its node. Requires a container runtime socket
//! (DOCKER_HOST honored).

use std::time::Duration;

use adapter_pg::PgPool;
use sqlx::migrate::MigrateError;
use uuid::Uuid;

use crate::commission_fill::{
    FILL_VERSION, LEVEL_TOKENS, commission_node, just_before_den_1, ledger_records, node_id_of,
    refusal, through_den_1,
};
use crate::migration_steps::{bare_pool, run_migrations_through, try_run_migrations_through};
use crate::node_backfill::{DEN_1_VERSION, archive, seed_commission};

/// The commission column drop's migration, as sqlx numbers it.
const DROP_VERSION: i64 = 20261022165210;

/// The title every planted row of the refusal-text test carries.
const CANARY_TITLE: &str = "A distinctive commission title";

/// The title the title plant of the refusal-text test sets apart from the node's name.
const CANARY_APART: &str = "A distinctive title set apart";

/// Sets the level of the node commission `$1` points at to Listed.
const WIDEN_NODE_TO_LISTED: &str =
    "UPDATE node SET level = 'listed' FROM commission c WHERE c.id = $1 AND node.id = c.node_id";

/// Sets commission `$1`'s own title to `$2`.
const RETITLE_COMMISSION: &str = "UPDATE commission SET title = $2 WHERE id = $1";

/// How many times a lock test polls for the drop to wait.
const LOCK_WAIT_POLLS: u32 = 500;

/// How long a lock test sleeps between two polls.
const LOCK_WAIT_INTERVAL: Duration = Duration::from_millis(20);

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

/// Set `commission`'s own title, a test-only update; no trigger copies a title onto a node.
async fn set_title_apart(pool: &PgPool, commission: Uuid, title: &str) {
    let updated = sqlx::query(RETITLE_COMMISSION)
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
    let updated = sqlx::query(WIDEN_NODE_TO_LISTED)
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
    execute_alone(
        pool,
        "ALTER TABLE commission DROP CONSTRAINT commission_has_its_node",
    )
    .await;
    let node_less = seed_commission(pool, title, "private").await;
    execute_alone(
        pool,
        "ALTER TABLE commission ADD CONSTRAINT commission_has_its_node \
         CHECK (node_id IS NOT NULL) NOT VALID",
    )
    .await;
    node_less
}

/// Run `statement` on its own, outside any transaction, in this test's own database.
async fn execute_alone(pool: &PgPool, statement: &'static str) {
    sqlx::query(statement)
        .execute(pool)
        .await
        .unwrap_or_else(|error| panic!("{statement}: {error}"));
}

/// Delete `commission`'s node by hand after the fill, in this test's own database only:
/// `commission_node_id_fkey` is dropped for the delete, then added back NOT VALID, since the
/// commission still points at the deleted id. Answers that id.
async fn plant_deleted_node(pool: &PgPool, commission: Uuid) -> Uuid {
    let node = node_id_of(pool, commission)
        .await
        .expect("the fill bound the commission to a node");
    execute_alone(
        pool,
        "ALTER TABLE commission DROP CONSTRAINT commission_node_id_fkey",
    )
    .await;
    let deleted = sqlx::query("DELETE FROM node WHERE id = $1")
        .bind(node)
        .execute(pool)
        .await
        .expect("delete the commission's node");
    assert_eq!(deleted.rows_affected(), 1, "one node is deleted");
    execute_alone(
        pool,
        "ALTER TABLE commission ADD CONSTRAINT commission_node_id_fkey \
         FOREIGN KEY (node_id) REFERENCES node (id) DEFERRABLE INITIALLY DEFERRED NOT VALID",
    )
    .await;
    node
}

/// Re-segment `commission`'s node to a fresh id after the fill, in this test's own database
/// only: `node_keeps_its_place` is disabled for the update and enabled again, so the node keeps
/// the commission's title and level but is no longer its own.
async fn plant_foreign_node(pool: &PgPool, commission: Uuid) {
    execute_alone(
        pool,
        "ALTER TABLE node DISABLE TRIGGER node_keeps_its_place",
    )
    .await;
    let updated = sqlx::query(
        "UPDATE node SET segment = $2 FROM commission c WHERE c.id = $1 AND node.id = c.node_id",
    )
    .bind(commission)
    .bind(Uuid::now_v7().to_string())
    .execute(pool)
    .await
    .expect("re-segment the commission's node");
    assert_eq!(updated.rows_affected(), 1, "one node is re-segmented");
    execute_alone(pool, "ALTER TABLE node ENABLE TRIGGER node_keeps_its_place").await;
}

/// Whether a statement in this test's own database waits for a lock on `commission` or `node`.
async fn a_lock_on_commission_or_node_waits(pool: &PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_locks l
                        WHERE NOT l.granted
                          AND l.database = (SELECT oid FROM pg_database
                                            WHERE datname = current_database())
                          AND l.relation IN ('commission'::regclass, 'node'::regclass))",
    )
    .fetch_one(pool)
    .await
    .expect("read pg_locks")
}

/// Polls until a statement in this test's own database waits for a lock on `commission` or
/// `node`, and fails the test if none does within `LOCK_WAIT_POLLS` polls (about ten seconds).
async fn wait_until_a_lock_waits(pool: &PgPool) {
    for _ in 0..LOCK_WAIT_POLLS {
        if a_lock_on_commission_or_node_waits(pool).await {
            return;
        }
        tokio::time::sleep(LOCK_WAIT_INTERVAL).await;
    }
    panic!("the drop never waited for a lock on commission or node while the editor held one");
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
async fn a_commission_whose_node_was_deleted_by_hand_stops_the_drop() {
    let (pool, _container) = through_den_1().await;
    seed_commission(&pool, "A kept commission", "private").await;
    let dangling = seed_commission(&pool, "A commission whose node goes", "private").await;
    run_migrations_through(&pool, FILL_VERSION).await;
    let deleted_node = plant_deleted_node(&pool, dangling).await;

    let stopped = try_run_migrations_through(&pool, DROP_VERSION)
        .await
        .expect_err("the drop stops");

    assert_stopped_naming(&pool, &stopped, dangling).await;
    assert_eq!(
        node_id_of(&pool, dangling).await,
        Some(deleted_node),
        "the commission still points at the deleted node"
    );
}

#[tokio::test]
async fn a_commission_bound_to_a_node_that_is_not_its_own_stops_the_drop() {
    let (pool, _container) = through_den_1().await;
    seed_commission(&pool, "A commission", "private").await;
    let foreign = seed_commission(&pool, "A commission", "private").await;
    run_migrations_through(&pool, FILL_VERSION).await;
    plant_foreign_node(&pool, foreign).await;

    let stopped = try_run_migrations_through(&pool, DROP_VERSION)
        .await
        .expect_err("the drop stops");

    assert_stopped_naming(&pool, &stopped, foreign).await;
}

#[tokio::test]
async fn a_level_edit_committed_while_the_drop_waits_stops_the_drop() {
    let (pool, _container) = through_den_1().await;
    let edited = seed_commission(&pool, "A commission edited mid-drop", "private").await;
    run_migrations_through(&pool, FILL_VERSION).await;
    let mut editor = pool.begin().await.expect("begin the editor");
    let widened = sqlx::query(WIDEN_NODE_TO_LISTED)
        .bind(edited)
        .execute(&mut *editor)
        .await
        .expect("widen the node, uncommitted");
    assert_eq!(widened.rows_affected(), 1, "one node is widened");
    let drop_pool = pool.clone();
    let drop =
        tokio::spawn(async move { try_run_migrations_through(&drop_pool, DROP_VERSION).await });

    wait_until_a_lock_waits(&pool).await;
    editor.commit().await.expect("commit the level edit");
    let stopped = drop
        .await
        .expect("join the drop")
        .expect_err("the drop stops");

    assert_stopped_naming(&pool, &stopped, edited).await;
    assert_eq!(
        commission_node(&pool, edited).await.level,
        "listed",
        "no level is re-synced"
    );
}

#[tokio::test]
async fn a_title_rename_committed_while_the_drop_waits_stops_the_drop() {
    let (pool, _container) = through_den_1().await;
    let edited = seed_commission(&pool, "A commission renamed mid-drop", "private").await;
    run_migrations_through(&pool, FILL_VERSION).await;
    let mut editor = pool.begin().await.expect("begin the editor");
    let renamed = sqlx::query(RETITLE_COMMISSION)
        .bind(edited)
        .bind("A title renamed mid-drop")
        .execute(&mut *editor)
        .await
        .expect("rename the commission's title, uncommitted");
    assert_eq!(renamed.rows_affected(), 1, "one commission is renamed");
    let drop_pool = pool.clone();
    let drop =
        tokio::spawn(async move { try_run_migrations_through(&drop_pool, DROP_VERSION).await });

    wait_until_a_lock_waits(&pool).await;
    editor.commit().await.expect("commit the title rename");
    let stopped = drop
        .await
        .expect("join the drop")
        .expect_err("the drop stops");

    assert_stopped_naming(&pool, &stopped, edited).await;
    assert_eq!(
        commission_node(&pool, edited).await.name,
        "A commission renamed mid-drop",
        "no title is copied onto the node"
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
    let node_deleted = {
        let (pool, container) = through_den_1().await;
        let commission = seed_commission(&pool, CANARY_TITLE, "private").await;
        run_migrations_through(&pool, FILL_VERSION).await;
        plant_deleted_node(&pool, commission).await;
        (pool, container)
    };

    for (label, (pool, _container)) in [
        ("a title that differs", title_apart),
        ("a level that differs", level_apart),
        ("a commission without a node", node_less),
        ("a commission whose node was deleted", node_deleted),
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
