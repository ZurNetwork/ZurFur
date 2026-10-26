//! The Slot title drop: every test seeds made-up commissions and Slots with raw SQL through the
//! real path (Slots made before the node table and Slots declared since on the old path), runs
//! the Slot node fill, plants at most one difference, then runs the drop and nothing after it.
//! Checks that each of these stops the drop, naming the Slot's element id alone, the drop
//! unrecorded and `title` still there: a Slot with no node, a `node_id` that names a missing node
//! or one that isn't the Slot's own (another segment, or under another commission), a title that
//! differs from its node's name, and a write committed while the drop waits for its lock on
//! `commission_slot` or `node`. A view reading `title` stops it too, and stays. Checks what the
//! drop leaves: no title column or its CHECK, `node_id` NOT NULL, the fill's transitional check
//! gone, and each Slot's name on its node. Requires a container runtime socket (DOCKER_HOST
//! honored).

use adapter_pg::PgPool;
use sqlx::migrate::MigrateError;
use uuid::Uuid;

use crate::commission_fill::{ledger_records, refusal};
use crate::migration_steps::{run_migrations_through, try_run_migrations_through};
use crate::node_backfill::seed_slot;
use crate::slot_fill::{
    CANARY_TITLE, SEEDED_SLOT_TITLES, SLOT_FILL_VERSION, bound_commission, execute_alone,
    rename_slot_node, seeded_through_den_1, slot_node, slot_node_id, slots_entry,
    wait_until_a_lock_on_waits,
};

/// The Slot title drop's migration, as sqlx numbers it.
const DROP_VERSION: i64 = 20261026131355;

/// The name a planted rename sets apart from a Slot's title; no refusal may echo it.
const CANARY_APART: &str = "A distinctive slot name set apart";

/// The title a planted retitle sets apart from a Slot's node's name; no refusal may echo it.
const TITLE_APART: &str = "A distinctive slot title set apart";

/// Sets the title of Slot `$1` to `$2`.
const RETITLE_SLOT: &str = "UPDATE commission_slot SET title = $2 WHERE element_id = $1";

/// PostgreSQL's SQLSTATE for a drop refused because other objects depend on what it drops.
const DEPENDENT_OBJECTS_STILL_EXIST: &str = "2BP01";

/// A database seeded through the real path and migrated through the Slot node fill, so every
/// Slot is bound to its own node. Answers the commission and every seeded Slot.
async fn through_the_fill() -> (PgPool, impl Sized, Uuid, Vec<Uuid>) {
    let (pool, container, commission, slots) = seeded_through_den_1().await;
    run_migrations_through(&pool, SLOT_FILL_VERSION).await;
    (pool, container, commission, slots)
}

/// Whether `commission_slot` has the column `column`.
async fn slot_has_column(pool: &PgPool, column: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns
                        WHERE table_name = 'commission_slot' AND column_name = $1)",
    )
    .bind(column)
    .fetch_one(pool)
    .await
    .expect("read the catalog's columns")
}

/// Whether the view `name` exists.
async fn view_exists(pool: &PgPool, name: &str) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_views WHERE viewname = $1)")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("read the catalog's views")
}

/// The SQLSTATE of the database's own refusal inside `stopped`.
fn refusal_code(stopped: &MigrateError) -> String {
    let (MigrateError::Execute(error) | MigrateError::ExecuteMigration(error, _)) = stopped else {
        panic!("the database refused the migration: {stopped:?}");
    };
    error
        .as_database_error()
        .and_then(|database_error| database_error.code())
        .map(|code| code.into_owned())
        .unwrap_or_else(|| panic!("the database refused the migration with a code: {stopped:?}"))
}

/// Whether `commission_slot.node_id` is NOT NULL.
async fn node_id_is_not_null(pool: &PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT attnotnull FROM pg_attribute
         WHERE attrelid = 'commission_slot'::regclass AND attname = 'node_id'",
    )
    .fetch_one(pool)
    .await
    .expect("read node_id's attribute")
}

/// Whether `commission_slot` carries the constraint `name`.
async fn slot_has_constraint(pool: &PgPool, name: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_constraint
                        WHERE conrelid = 'commission_slot'::regclass AND conname = $1)",
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .expect("read the catalog's constraints")
}

/// How many CHECK constraints on `commission_slot` read `title`.
async fn title_checks(pool: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM pg_constraint
         WHERE conrelid = 'commission_slot'::regclass AND contype = 'c'
           AND pg_get_constraintdef(oid) LIKE '%title%'",
    )
    .fetch_one(pool)
    .await
    .expect("read the catalog's checks")
}

/// Seed a Slot with no node on `commission` after the fill, in this test's own database only:
/// `commission_slot_has_its_node` is dropped for the insert, then added back exactly as the fill
/// added it, so the schema is the fill's and only the row differs. Answers its element id.
async fn plant_node_less(pool: &PgPool, commission: Uuid, title: &str) -> Uuid {
    execute_alone(
        pool,
        "ALTER TABLE commission_slot DROP CONSTRAINT commission_slot_has_its_node",
    )
    .await;
    let node_less = seed_slot(pool, commission, title).await;
    execute_alone(
        pool,
        "ALTER TABLE commission_slot ADD CONSTRAINT commission_slot_has_its_node \
         CHECK (node_id IS NOT NULL) NOT VALID",
    )
    .await;
    node_less
}

/// Delete the node Slot `element` points at, in this test's own database only:
/// `commission_slot_node_id_fkey` is dropped for the delete, then added back NOT VALID, since the
/// Slot still points at the deleted id. Answers that id.
async fn plant_dangling(pool: &PgPool, element: Uuid) -> Uuid {
    let node = slot_node_id(pool, element)
        .await
        .expect("the fill bound the Slot to a node");
    execute_alone(
        pool,
        "ALTER TABLE commission_slot DROP CONSTRAINT commission_slot_node_id_fkey",
    )
    .await;
    let deleted = sqlx::query("DELETE FROM node WHERE id = $1")
        .bind(node)
        .execute(pool)
        .await
        .expect("delete the Slot's node");
    assert_eq!(deleted.rows_affected(), 1, "one node is deleted");
    execute_alone(
        pool,
        "ALTER TABLE commission_slot ADD CONSTRAINT commission_slot_node_id_fkey \
         FOREIGN KEY (node_id) REFERENCES node (id) DEFERRABLE INITIALLY DEFERRED NOT VALID",
    )
    .await;
    node
}

/// Re-segment the node Slot `element` points at to a fresh id, in this test's own database
/// only: `node_keeps_its_place` is disabled for the update and enabled again, so the node keeps
/// the Slot's title as its name and its place under the commission's `slots` entry, but is no
/// longer the Slot's own.
async fn plant_foreign(pool: &PgPool, element: Uuid) {
    execute_alone(
        pool,
        "ALTER TABLE node DISABLE TRIGGER node_keeps_its_place",
    )
    .await;
    let updated = sqlx::query(
        "UPDATE node SET segment = $2 FROM commission_slot s
         WHERE s.element_id = $1 AND node.id = s.node_id",
    )
    .bind(element)
    .bind(Uuid::now_v7().to_string())
    .execute(pool)
    .await
    .expect("re-segment the Slot's node");
    assert_eq!(updated.rows_affected(), 1, "one node is re-segmented");
    execute_alone(pool, "ALTER TABLE node ENABLE TRIGGER node_keeps_its_place").await;
}

/// Move the node Slot `element` points at under the `slots` entry of a second commission bound
/// to its own node, in this test's own database only: `node_keeps_its_place` is disabled for the
/// update and enabled again, so the node keeps its type, segment and name, and only its
/// commission differs.
async fn plant_under_another_commission(pool: &PgPool, element: Uuid) {
    let other_commission = bound_commission(pool, "Another commission").await;
    let other_slots = slots_entry(pool, other_commission).await;
    execute_alone(
        pool,
        "ALTER TABLE node DISABLE TRIGGER node_keeps_its_place",
    )
    .await;
    let moved = sqlx::query(
        "UPDATE node SET parent = $2 FROM commission_slot s
         WHERE s.element_id = $1 AND node.id = s.node_id",
    )
    .bind(element)
    .bind(other_slots)
    .execute(pool)
    .await
    .expect("move the Slot's node under another commission");
    assert_eq!(moved.rows_affected(), 1, "one node is moved");
    execute_alone(pool, "ALTER TABLE node ENABLE TRIGGER node_keeps_its_place").await;
}

/// Set Slot `element`'s own title apart from its node's name, a test-only update; nothing
/// copies a title onto a node.
async fn retitle_slot(pool: &PgPool, element: Uuid) {
    let retitled = sqlx::query(RETITLE_SLOT)
        .bind(element)
        .bind(TITLE_APART)
        .execute(pool)
        .await
        .expect("set the Slot's title apart");
    assert_eq!(retitled.rows_affected(), 1, "one Slot is retitled");
}

/// Asserts the drop stopped naming `named`, left no ledger row, and left the title column,
/// the nullable `node_id` and the fill's check.
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
        slot_has_column(pool, "title").await,
        "the title column still exists"
    );
    assert!(
        !node_id_is_not_null(pool).await,
        "node_id is still nullable"
    );
    assert!(
        slot_has_constraint(pool, "commission_slot_has_its_node").await,
        "the fill's check still stands"
    );
}

/// Asserts `message` is fixed lowercase text and ids, and that neither it nor the rest of
/// `stopped` echoes a canary title or name.
fn assert_names_ids_only(label: &str, stopped: &MigrateError) {
    let message = refusal(stopped);
    let everything_it_says = format!("{message} {stopped} {stopped:?}");
    for canary in [CANARY_TITLE, CANARY_APART, TITLE_APART] {
        assert!(
            !everything_it_says.contains(canary),
            "{label}: the refusal never echoes a title or a name: {everything_it_says}"
        );
    }
    let names_no_other_text = message
        .split_whitespace()
        .filter(|word| word.parse::<Uuid>().is_err())
        .all(|word| !word.chars().any(char::is_uppercase));
    assert!(
        names_no_other_text,
        "{label}: the refusal is fixed text and ids: {message}"
    );
}

// ─── What passes ───────────────────────────────────────────────────────────

#[tokio::test]
async fn after_the_drop_title_is_gone_and_node_id_is_not_null() {
    let (pool, _container, _commission, slots) = through_the_fill().await;
    assert_eq!(
        title_checks(&pool).await,
        1,
        "the title's CHECK stands before"
    );

    run_migrations_through(&pool, DROP_VERSION).await;

    assert!(
        ledger_records(&pool, DROP_VERSION).await,
        "the drop is recorded"
    );
    assert!(
        !slot_has_column(&pool, "title").await,
        "the title column is gone"
    );
    assert_eq!(
        title_checks(&pool).await,
        0,
        "the title's CHECK went with it"
    );
    assert!(node_id_is_not_null(&pool).await, "node_id is NOT NULL");
    assert!(
        !slot_has_constraint(&pool, "commission_slot_has_its_node").await,
        "the fill's NOT VALID check is gone"
    );
    for (slot, title) in slots.iter().zip(SEEDED_SLOT_TITLES) {
        assert_eq!(
            slot_node(&pool, *slot).await.name,
            title,
            "{slot} keeps its name on its node"
        );
    }
}

// ─── Its refusals ──────────────────────────────────────────────────────────

#[tokio::test]
async fn a_slot_without_a_node_stops_the_drop() {
    let (pool, _container, commission, _slots) = through_the_fill().await;
    let node_less = plant_node_less(&pool, commission, "A node-less Slot").await;

    let stopped = try_run_migrations_through(&pool, DROP_VERSION)
        .await
        .expect_err("the drop stops");

    assert_stopped_naming(&pool, &stopped, node_less).await;
    assert_eq!(
        slot_node_id(&pool, node_less).await,
        None,
        "the planted Slot still has no node"
    );
}

#[tokio::test]
async fn a_dangling_or_foreign_node_id_stops_the_drop() {
    let (dangling_db, _dangling_container, _, dangling_slots) = through_the_fill().await;
    let dangling = dangling_slots[1];
    let deleted_node = plant_dangling(&dangling_db, dangling).await;
    let (foreign_db, _foreign_container, _, foreign_slots) = through_the_fill().await;
    let foreign = foreign_slots[3];
    plant_foreign(&foreign_db, foreign).await;
    let (moved_db, _moved_container, _, moved_slots) = through_the_fill().await;
    let moved = moved_slots[2];
    plant_under_another_commission(&moved_db, moved).await;

    for (label, pool, named) in [
        ("a dangling node_id", &dangling_db, dangling),
        ("a foreign node_id", &foreign_db, foreign),
        ("a node under another commission", &moved_db, moved),
    ] {
        let stopped = try_run_migrations_through(pool, DROP_VERSION)
            .await
            .expect_err("the drop stops");
        let message = refusal(&stopped);
        assert!(
            message.contains("no node of its own"),
            "{label}: the own-node check refuses: {message}"
        );
        assert_stopped_naming(pool, &stopped, named).await;
    }
    assert_eq!(
        slot_node_id(&dangling_db, dangling).await,
        Some(deleted_node),
        "the Slot still points at the deleted node"
    );
}

// The drop carries no CASCADE: a view reading `title` makes PostgreSQL refuse it, and the view
// stays.
#[tokio::test]
async fn a_view_reading_title_stops_the_drop_and_stays() {
    let (pool, _container, _commission, _slots) = through_the_fill().await;
    execute_alone(
        &pool,
        "CREATE VIEW slot_titles AS SELECT element_id, title FROM commission_slot",
    )
    .await;

    let stopped = try_run_migrations_through(&pool, DROP_VERSION)
        .await
        .expect_err("the drop stops");

    assert_eq!(
        refusal_code(&stopped),
        DEPENDENT_OBJECTS_STILL_EXIST,
        "PostgreSQL refuses to drop a column a view reads: {stopped}"
    );
    assert!(
        !ledger_records(&pool, DROP_VERSION).await,
        "the ledger does not record the stopped drop"
    );
    assert!(
        slot_has_column(&pool, "title").await,
        "the title column still exists"
    );
    assert!(
        view_exists(&pool, "slot_titles").await,
        "the view reading title still exists"
    );
}

#[tokio::test]
async fn a_title_that_differs_stops_the_drop() {
    let (pool, _container, _commission, slots) = through_the_fill().await;
    let differing = slots[1];
    rename_slot_node(&pool, differing, "Another name").await;

    let stopped = try_run_migrations_through(&pool, DROP_VERSION)
        .await
        .expect_err("the drop stops");

    assert_stopped_naming(&pool, &stopped, differing).await;
    assert_eq!(
        slot_node(&pool, differing).await.name,
        "Another name",
        "nothing is re-synced"
    );
}

#[tokio::test]
async fn the_refusals_name_ids_only_and_title_still_exists() {
    let (node_less_db, _node_less_container, node_less_commission, _) = through_the_fill().await;
    let node_less = plant_node_less(&node_less_db, node_less_commission, CANARY_TITLE).await;
    let (dangling_db, _dangling_container, _, dangling_slots) = through_the_fill().await;
    let dangling = dangling_slots[0];
    plant_dangling(&dangling_db, dangling).await;
    let (foreign_db, _foreign_container, _, foreign_slots) = through_the_fill().await;
    let foreign = foreign_slots[0];
    plant_foreign(&foreign_db, foreign).await;
    let (differing_db, _differing_container, _, differing_slots) = through_the_fill().await;
    let differing = differing_slots[2];
    rename_slot_node(&differing_db, differing, CANARY_APART).await;

    for (label, pool, named) in [
        ("a Slot without a node", &node_less_db, node_less),
        ("a dangling node_id", &dangling_db, dangling),
        ("a foreign node_id", &foreign_db, foreign),
        ("a title that differs", &differing_db, differing),
    ] {
        let stopped = try_run_migrations_through(pool, DROP_VERSION)
            .await
            .expect_err("the drop stops");
        assert_names_ids_only(label, &stopped);
        assert_stopped_naming(pool, &stopped, named).await;
    }
}

// ─── Its locks ─────────────────────────────────────────────────────────────

// A Slot's title a writer changes and commits while the drop waits on `commission_slot` is what
// the drop's check reads: the title now differs from its node's name, and the drop stops naming
// the Slot.
#[tokio::test]
async fn a_slot_title_committed_while_the_drop_waits_on_commission_slot_stops_it() {
    let (pool, _container, _commission, slots) = through_the_fill().await;
    let edited = slots[1];
    let mut writer = pool.begin().await.expect("begin the writer");
    let retitled = sqlx::query(RETITLE_SLOT)
        .bind(edited)
        .bind(TITLE_APART)
        .execute(&mut *writer)
        .await
        .expect("retitle the Slot, uncommitted");
    assert_eq!(retitled.rows_affected(), 1, "one Slot is retitled");
    let drop_pool = pool.clone();
    let drop =
        tokio::spawn(async move { try_run_migrations_through(&drop_pool, DROP_VERSION).await });

    wait_until_a_lock_on_waits(&pool, "commission_slot").await;
    writer.commit().await.expect("commit the title");
    let stopped = drop
        .await
        .expect("join the drop")
        .expect_err("the drop stops");

    assert_stopped_naming(&pool, &stopped, edited).await;
}

// A Slot node's name a writer changes and commits while the drop waits on `node` is what the
// drop's check reads: the name now differs from the Slot's title, and the drop stops naming the
// Slot.
#[tokio::test]
async fn a_slot_node_rename_committed_while_the_drop_waits_on_node_stops_it() {
    let (pool, _container, _commission, slots) = through_the_fill().await;
    let edited = slots[3];
    let mut writer = pool.begin().await.expect("begin the writer");
    let renamed = sqlx::query(
        "UPDATE node SET name = 'Renamed mid-drop' FROM commission_slot s
         WHERE s.element_id = $1 AND node.id = s.node_id",
    )
    .bind(edited)
    .execute(&mut *writer)
    .await
    .expect("rename the Slot's node, uncommitted");
    assert_eq!(renamed.rows_affected(), 1, "one Slot node is renamed");
    let drop_pool = pool.clone();
    let drop =
        tokio::spawn(async move { try_run_migrations_through(&drop_pool, DROP_VERSION).await });

    wait_until_a_lock_on_waits(&pool, "node").await;
    writer.commit().await.expect("commit the rename");
    let stopped = drop
        .await
        .expect("join the drop")
        .expect_err("the drop stops");

    assert_stopped_naming(&pool, &stopped, edited).await;
    assert_eq!(
        slot_node(&pool, edited).await.name,
        "Renamed mid-drop",
        "nothing is re-synced"
    );
}

// ─── The seeded run ────────────────────────────────────────────────────────

// Through the real path: a clean run drops the title with every Slot's name kept on its node;
// each plant (a differing title, a NULL node_id, a dangling node_id) stops the migration naming
// its id alone, never the canary title, and `title` still stands.
#[tokio::test]
async fn a_seeded_database_runs_the_drop_through_the_real_path() {
    let (clean, _clean_container, _, clean_slots) = through_the_fill().await;

    run_migrations_through(&clean, DROP_VERSION).await;

    assert!(!slot_has_column(&clean, "title").await);
    for (slot, title) in clean_slots.iter().zip(SEEDED_SLOT_TITLES) {
        assert_eq!(slot_node(&clean, *slot).await.name, title);
    }

    let (differing_db, _differing_container, _, differing_slots) = through_the_fill().await;
    let differing = differing_slots[0];
    retitle_slot(&differing_db, differing).await;
    let (node_less_db, _node_less_container, node_less_commission, _) = through_the_fill().await;
    let node_less = plant_node_less(&node_less_db, node_less_commission, CANARY_TITLE).await;
    let (dangling_db, _dangling_container, _, dangling_slots) = through_the_fill().await;
    let dangling = dangling_slots[2];
    plant_dangling(&dangling_db, dangling).await;

    for (label, pool, named) in [
        ("a differing title", &differing_db, differing),
        ("a NULL node_id", &node_less_db, node_less),
        ("a dangling node_id", &dangling_db, dangling),
    ] {
        let stopped = try_run_migrations_through(pool, DROP_VERSION)
            .await
            .expect_err("the drop stops");
        assert_names_ids_only(label, &stopped);
        assert_stopped_naming(pool, &stopped, named).await;
    }
}
