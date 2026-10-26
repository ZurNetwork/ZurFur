//! The Slot node fill: every test seeds made-up rows with raw SQL on a database migrated up to
//! just before the fill (some before the node table, so its own backfill binds them; some
//! after, on the old path, with no node), then runs the fill and nothing after it. Checks the
//! nodes it writes and binds, that it binds only the nodes its own statement made, its
//! refusals (each naming an id alone, the fill unrecorded and every node-less Slot still without
//! a node), the locks it takes on `commission`, `commission_slot` and `node` (a writer's row
//! committed while the fill waits is what its check reads), the seeded run through the real
//! path, and the guards it adds. Requires a container runtime socket (DOCKER_HOST honored).

use std::time::Duration;

use adapter_pg::PgPool;
use sqlx::migrate::MigrateError;
use uuid::Uuid;

use crate::migration_steps::{
    bare_pool, run_migrations_before, run_migrations_through, try_run_migrations_through,
};
use crate::node_backfill::{
    DEN_1_VERSION, NodeRow, OWNER_DID, every_node, seed_commission, seed_owner, seed_slot,
};

/// The Slot node fill's migration, as sqlx numbers it.
pub(crate) const SLOT_FILL_VERSION: i64 = 20261026080502;

/// A title no refusal may echo.
pub(crate) const CANARY_TITLE: &str = "Canary slot title 7e3c";

/// The title of each Slot `seeded_through_den_1` seeds, in the order it answers them: two made
/// before the node table, then two declared since.
pub(crate) const SEEDED_SLOT_TITLES: [&str; 4] = [
    CANARY_TITLE,
    "A second Slot from before",
    CANARY_TITLE,
    "A second Slot declared since",
];

/// How many times a lock test polls `pg_locks` for the fill's wait before failing.
const LOCK_WAIT_POLLS: u32 = 500;

/// How long a lock test waits between two polls.
const LOCK_WAIT_INTERVAL: Duration = Duration::from_millis(20);

/// A fresh database migrated up to just before the node table, with its owner.
async fn just_before_den_1() -> (PgPool, impl Sized) {
    let (pool, container) = bare_pool().await;
    run_migrations_before(&pool, DEN_1_VERSION).await;
    seed_owner(&pool).await;
    (pool, container)
}

/// A fresh database migrated up to just before the Slot fill, with its owner.
async fn just_before_the_fill() -> (PgPool, impl Sized) {
    let (pool, container) = just_before_den_1().await;
    run_migrations_before(&pool, SLOT_FILL_VERSION).await;
    (pool, container)
}

/// A commission bound to its own node and six entries, inserted in one committed unit on the
/// schema before the fill, as the den's write gate writes a create. Answers its id.
pub(crate) async fn bound_commission(pool: &PgPool, title: &str) -> Uuid {
    let commission = Uuid::now_v7();
    let node = Uuid::now_v7();
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query(
        "INSERT INTO node (id, parent, segment, type, level, name)
         VALUES ($1, NULL, $2, 'commission', 'private', $3)",
    )
    .bind(node)
    .bind(commission.to_string())
    .bind(title)
    .execute(&mut *tx)
    .await
    .expect("insert the commission's node");
    sqlx::query(
        "INSERT INTO node (id, parent, segment, type, level, name)
         SELECT gen_random_uuid(), $1, entry.word, 'commission.' || entry.word, 'private', entry.word
         FROM (VALUES ('attachments'), ('products'), ('slots'), ('changelog'), ('participants'),
                      ('brief')) AS entry (word)",
    )
    .bind(node)
    .execute(&mut *tx)
    .await
    .expect("insert its six entries");
    sqlx::query(
        "INSERT INTO commission (id, node_id, owner_id, lifecycle, created_at)
         VALUES ($1, $2, $3, 'draft', now())",
    )
    .bind(commission)
    .bind(node)
    .bind(OWNER_DID)
    .execute(&mut *tx)
    .await
    .expect("insert the commission at its node");
    tx.commit().await.expect("the bound commission commits");
    commission
}

/// The node a Slot row points at, or `None`.
pub(crate) async fn slot_node_id(pool: &PgPool, element: Uuid) -> Option<Uuid> {
    sqlx::query_scalar("SELECT node_id FROM commission_slot WHERE element_id = $1")
        .bind(element)
        .fetch_one(pool)
        .await
        .expect("read a Slot's node_id")
}

/// The node a Slot row points at.
pub(crate) async fn slot_node(pool: &PgPool, element: Uuid) -> NodeRow {
    sqlx::query_as(
        "SELECT n.* FROM node n JOIN commission_slot s ON s.node_id = n.id WHERE s.element_id = $1",
    )
    .bind(element)
    .fetch_one(pool)
    .await
    .expect("read a Slot's node")
}

/// The `slots` entry of `commission`'s node.
pub(crate) async fn slots_entry(pool: &PgPool, commission: Uuid) -> Uuid {
    sqlx::query_scalar(
        "SELECT e.id FROM node e JOIN commission c ON e.parent = c.node_id
         WHERE c.id = $1 AND e.type = 'commission.slots'",
    )
    .bind(commission)
    .fetch_one(pool)
    .await
    .expect("read a commission's slots entry")
}

/// Insert a bare `slot` node under `commission`'s `slots` entry that no Slot points at, named
/// `name`, committed on the schema before the fill.
async fn phantom_slot_node(pool: &PgPool, commission: Uuid, name: &str) -> Uuid {
    let parent = slots_entry(pool, commission).await;
    let node = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO node (id, parent, segment, type, level, name)
         VALUES ($1, $2, $3, 'slot', 'private', $4)",
    )
    .bind(node)
    .bind(parent)
    .bind(Uuid::now_v7().to_string())
    .bind(name)
    .execute(pool)
    .await
    .expect("insert a phantom slot node");
    node
}

/// Rename the node the bound Slot `element` points at to `name`, apart from its title.
pub(crate) async fn rename_slot_node(pool: &PgPool, element: Uuid, name: &str) {
    let renamed = sqlx::query(
        "UPDATE node SET name = $2 FROM commission_slot s
         WHERE s.element_id = $1 AND node.id = s.node_id",
    )
    .bind(element)
    .bind(name)
    .execute(pool)
    .await
    .expect("rename the Slot's node");
    assert_eq!(renamed.rows_affected(), 1, "one Slot node is renamed");
}

/// Whether the ledger records the fill.
async fn fill_recorded(pool: &PgPool) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = $1)")
        .bind(SLOT_FILL_VERSION)
        .fetch_one(pool)
        .await
        .expect("read the ledger")
}

/// How many `slot` nodes no Slot points at.
async fn phantom_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM node n WHERE n.type = 'slot'
         AND NOT EXISTS (SELECT 1 FROM commission_slot s WHERE s.node_id = n.id)",
    )
    .fetch_one(pool)
    .await
    .expect("count the phantom slot nodes")
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
        .unwrap_or_else(|| panic!("the database refused the fill: {stopped:?}"))
}

/// Asserts the fill stopped naming `named`, left no ledger row, and left each of `node_less`
/// without a node.
async fn assert_stopped_naming(
    pool: &PgPool,
    stopped: &MigrateError,
    named: Uuid,
    node_less: &[Uuid],
) {
    let message = refusal(stopped);
    assert!(
        message.contains(&named.to_string()),
        "the refusal names {named}: {message}"
    );
    assert!(
        !fill_recorded(pool).await,
        "the ledger does not record the stopped fill"
    );
    for slot in node_less {
        assert_eq!(slot_node_id(pool, *slot).await, None, "{slot} has no node");
    }
}

/// Run `statement` in a transaction of its own, committed.
pub(crate) async fn execute_alone(pool: &PgPool, statement: &'static str) {
    sqlx::query(statement)
        .execute(pool)
        .await
        .unwrap_or_else(|error| panic!("{statement}: {error:?}"));
}

/// Whether a statement in this test's own database waits for a lock on `table`.
async fn a_lock_on_waits(pool: &PgPool, table: &str) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_locks l
                        WHERE NOT l.granted
                          AND l.database = (SELECT oid FROM pg_database
                                            WHERE datname = current_database())
                          AND l.relation = $1::regclass)",
    )
    .bind(table)
    .fetch_one(pool)
    .await
    .expect("read pg_locks")
}

/// Polls until a statement in this test's own database waits for a lock on `table`, and fails
/// the test if none does within `LOCK_WAIT_POLLS` polls (about ten seconds).
pub(crate) async fn wait_until_a_lock_on_waits(pool: &PgPool, table: &str) {
    for _ in 0..LOCK_WAIT_POLLS {
        if a_lock_on_waits(pool, table).await {
            return;
        }
        tokio::time::sleep(LOCK_WAIT_INTERVAL).await;
    }
    panic!("the migration never waited for a lock on {table} while the writer held one");
}

// ─── The fill ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_slot_declared_since_den_1_gets_its_node() {
    let (pool, _container) = just_before_den_1().await;
    let commission = seed_commission(&pool, "A commission", "private").await;
    let old = seed_slot(&pool, commission, "A Slot from before").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    let declared = seed_slot(&pool, commission, "A Slot declared since").await;
    run_migrations_before(&pool, SLOT_FILL_VERSION).await;
    let old_node = slot_node_id(&pool, old).await;

    run_migrations_through(&pool, SLOT_FILL_VERSION).await;

    let node = slot_node(&pool, declared).await;
    let expected = NodeRow {
        id: node.id,
        parent: Some(slots_entry(&pool, commission).await),
        segment: declared.to_string(),
        node_type: "slot".to_owned(),
        level: "private".to_owned(),
        name: "A Slot declared since".to_owned(),
    };
    assert_eq!(node, expected);
    assert_eq!(
        slot_node_id(&pool, old).await,
        old_node,
        "the bound Slot keeps its node"
    );
    assert_eq!(phantom_count(&pool).await, 0);
    assert!(fill_recorded(&pool).await);
}

// The fill binds a node-less Slot only to the node its own statement made: a node elsewhere
// holding the Slot's element id as its segment, here a commission's own node, is never adopted.
#[tokio::test]
async fn the_fill_binds_only_nodes_it_inserted() {
    let (pool, _container) = just_before_the_fill().await;
    let commission = bound_commission(&pool, "A commission").await;
    let declared = seed_slot(&pool, commission, "A Slot declared on the old path").await;
    let lookalike_node = Uuid::now_v7();
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query(
        "INSERT INTO node (id, parent, segment, type, level, name)
         VALUES ($1, NULL, $2, 'commission', 'private', 'A lookalike')",
    )
    .bind(lookalike_node)
    .bind(declared.to_string())
    .execute(&mut *tx)
    .await
    .expect("a commission node holding the Slot's element id");
    sqlx::query(
        "INSERT INTO node (id, parent, segment, type, level, name)
         SELECT gen_random_uuid(), $1, entry.word, 'commission.' || entry.word, 'private', entry.word
         FROM (VALUES ('attachments'), ('products'), ('slots'), ('changelog'), ('participants'),
                      ('brief')) AS entry (word)",
    )
    .bind(lookalike_node)
    .execute(&mut *tx)
    .await
    .expect("its entries");
    sqlx::query(
        "INSERT INTO commission (id, node_id, owner_id, lifecycle, created_at)
         VALUES ($1, $2, $3, 'draft', now())",
    )
    .bind(declared)
    .bind(lookalike_node)
    .bind(OWNER_DID)
    .execute(&mut *tx)
    .await
    .expect("a commission whose id is the Slot's element id");
    tx.commit().await.expect("the lookalike commits");
    let before: Vec<Uuid> = every_node(&pool).await.iter().map(|node| node.id).collect();

    run_migrations_through(&pool, SLOT_FILL_VERSION).await;

    let node = slot_node(&pool, declared).await;
    assert!(
        !before.contains(&node.id),
        "the Slot's node is one the fill made"
    );
    assert_eq!(node.node_type, "slot");
    assert_eq!(node.parent, Some(slots_entry(&pool, commission).await));
    let after = every_node(&pool).await;
    assert_eq!(after.len(), before.len() + 1, "the fill made one node");
}

// ─── Its refusals ──────────────────────────────────────────────────────────

#[tokio::test]
async fn a_phantom_slot_node_stops_it_naming_the_node() {
    let (pool, _container) = just_before_the_fill().await;
    let commission = bound_commission(&pool, "A commission").await;
    let node_less = seed_slot(&pool, commission, "A node-less Slot").await;
    let phantom = phantom_slot_node(&pool, commission, "A phantom").await;

    let stopped = try_run_migrations_through(&pool, SLOT_FILL_VERSION)
        .await
        .expect_err("the fill stops");

    assert_stopped_naming(&pool, &stopped, phantom, &[node_less]).await;
}

#[tokio::test]
async fn a_differing_name_stops_it_naming_the_slot() {
    let (pool, _container) = just_before_den_1().await;
    let commission = seed_commission(&pool, "A commission", "private").await;
    let differing = seed_slot(&pool, commission, "A title").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    let node_less = seed_slot(&pool, commission, "A node-less Slot").await;
    run_migrations_before(&pool, SLOT_FILL_VERSION).await;
    rename_slot_node(&pool, differing, "Another name").await;

    let stopped = try_run_migrations_through(&pool, SLOT_FILL_VERSION)
        .await
        .expect_err("the fill stops");

    assert_stopped_naming(&pool, &stopped, differing, &[node_less]).await;
    assert_eq!(
        slot_node(&pool, differing).await.name,
        "Another name",
        "nothing is re-synced"
    );
}

#[tokio::test]
async fn a_slot_whose_commission_has_no_slots_entry_stops_it_naming_the_slot() {
    let (pool, _container) = just_before_the_fill().await;
    let commission = bound_commission(&pool, "A commission").await;
    let node_less = seed_slot(&pool, commission, "A node-less Slot").await;
    let entry = slots_entry(&pool, commission).await;
    execute_alone(
        &pool,
        "ALTER TABLE node DISABLE TRIGGER node_entry_stays_with_its_commission",
    )
    .await;
    sqlx::query("DELETE FROM node WHERE id = $1")
        .bind(entry)
        .execute(&pool)
        .await
        .expect("delete the slots entry, in this test's database only");
    execute_alone(
        &pool,
        "ALTER TABLE node ENABLE TRIGGER node_entry_stays_with_its_commission",
    )
    .await;

    let stopped = try_run_migrations_through(&pool, SLOT_FILL_VERSION)
        .await
        .expect_err("the fill stops");

    assert_stopped_naming(&pool, &stopped, node_less, &[node_less]).await;
}

#[tokio::test]
async fn the_refusals_name_ids_only() {
    let (phantom_pool, _phantom_container) = just_before_the_fill().await;
    let phantom_commission = bound_commission(&phantom_pool, CANARY_TITLE).await;
    seed_slot(&phantom_pool, phantom_commission, CANARY_TITLE).await;
    let phantom = phantom_slot_node(&phantom_pool, phantom_commission, CANARY_TITLE).await;
    let (differing_pool, _differing_container) = just_before_den_1().await;
    let differing_commission = seed_commission(&differing_pool, CANARY_TITLE, "private").await;
    let differing = seed_slot(&differing_pool, differing_commission, CANARY_TITLE).await;
    run_migrations_before(&differing_pool, SLOT_FILL_VERSION).await;
    rename_slot_node(&differing_pool, differing, "A renamed Slot").await;

    for (label, pool, named) in [
        ("a phantom slot node", &phantom_pool, phantom),
        ("a differing name", &differing_pool, differing),
    ] {
        let stopped = try_run_migrations_through(pool, SLOT_FILL_VERSION)
            .await
            .expect_err("the fill stops");
        let message = refusal(&stopped);
        let everything_it_says = format!("{message} {stopped} {stopped:?}");
        assert!(
            message.contains(&named.to_string()),
            "{label}: the refusal names {named}: {message}"
        );
        assert!(
            !everything_it_says.contains(CANARY_TITLE),
            "{label}: the refusal never echoes a title or a name: {everything_it_says}"
        );
        assert!(
            !everything_it_says.contains("A renamed Slot"),
            "{label}: the refusal never echoes a node's name: {everything_it_says}"
        );
        let names_no_other_text = message
            .split_whitespace()
            .filter(|word| word.parse::<Uuid>().is_err())
            .all(|word| !word.chars().any(char::is_uppercase));
        assert!(
            names_no_other_text,
            "{label}: the refusal is fixed text and ids: {message}"
        );
        assert!(!fill_recorded(pool).await, "{label}: unrecorded");
    }
}

// ─── Its locks ─────────────────────────────────────────────────────────────

// A phantom slot node a writer inserts and commits while the fill waits on `node` is what the
// fill's check reads: the fill stops naming it.
#[tokio::test]
async fn a_slot_node_committed_while_the_fill_waits_on_node_stops_it() {
    let (pool, _container) = just_before_the_fill().await;
    let commission = bound_commission(&pool, "A commission").await;
    let parent = slots_entry(&pool, commission).await;
    let phantom = Uuid::now_v7();
    let mut writer = pool.begin().await.expect("begin the writer");
    sqlx::query(
        "INSERT INTO node (id, parent, segment, type, level, name)
         VALUES ($1, $2, $3, 'slot', 'private', 'A phantom')",
    )
    .bind(phantom)
    .bind(parent)
    .bind(Uuid::now_v7().to_string())
    .execute(&mut *writer)
    .await
    .expect("insert a slot node, uncommitted");
    let fill_pool = pool.clone();
    let fill =
        tokio::spawn(
            async move { try_run_migrations_through(&fill_pool, SLOT_FILL_VERSION).await },
        );

    wait_until_a_lock_on_waits(&pool, "node").await;
    writer.commit().await.expect("commit the slot node");
    let stopped = fill
        .await
        .expect("join the fill")
        .expect_err("the fill stops");

    assert_stopped_naming(&pool, &stopped, phantom, &[]).await;
}

// A Slot's title a writer changes and commits while the fill waits on `commission_slot` is what
// the fill's check reads: the bound Slot's node's name now differs, and the fill stops naming it.
#[tokio::test]
async fn a_slot_title_committed_while_the_fill_waits_on_commission_slot_stops_it() {
    let (pool, _container) = just_before_den_1().await;
    let commission = seed_commission(&pool, "A commission", "private").await;
    let bound = seed_slot(&pool, commission, "A title").await;
    run_migrations_before(&pool, SLOT_FILL_VERSION).await;
    let mut writer = pool.begin().await.expect("begin the writer");
    let retitled =
        sqlx::query("UPDATE commission_slot SET title = 'Another title' WHERE element_id = $1")
            .bind(bound)
            .execute(&mut *writer)
            .await
            .expect("retitle the Slot, uncommitted");
    assert_eq!(retitled.rows_affected(), 1, "one Slot is retitled");
    let fill_pool = pool.clone();
    let fill =
        tokio::spawn(
            async move { try_run_migrations_through(&fill_pool, SLOT_FILL_VERSION).await },
        );

    wait_until_a_lock_on_waits(&pool, "commission_slot").await;
    writer.commit().await.expect("commit the title");
    let stopped = fill
        .await
        .expect("join the fill")
        .expect_err("the fill stops");

    assert_stopped_naming(&pool, &stopped, bound, &[]).await;
}

// A commission a writer re-points and commits while the fill waits on `commission` is what the
// fill's check reads: its node-less Slot now has no `slots` entry to sit under, and the fill
// stops naming it. The re-point skips the guards that would refuse it, in this test's database
// only, so only the commission row changes.
#[tokio::test]
async fn a_commission_committed_while_the_fill_waits_on_commission_stops_it() {
    let (pool, _container) = just_before_the_fill().await;
    let commission = bound_commission(&pool, "A commission").await;
    let node_less = seed_slot(&pool, commission, "A node-less Slot").await;
    let brief: Uuid = sqlx::query_scalar(
        "SELECT e.id FROM node e JOIN commission c ON e.parent = c.node_id
         WHERE c.id = $1 AND e.type = 'commission.brief'",
    )
    .bind(commission)
    .fetch_one(&pool)
    .await
    .expect("read the commission's brief entry");
    execute_alone(
        &pool,
        "ALTER TABLE commission DISABLE TRIGGER node_id_is_set_once",
    )
    .await;
    execute_alone(
        &pool,
        "ALTER TABLE commission DISABLE TRIGGER node_matches_its_thing",
    )
    .await;
    let mut writer = pool.begin().await.expect("begin the writer");
    let repointed = sqlx::query("UPDATE commission SET node_id = $2 WHERE id = $1")
        .bind(commission)
        .bind(brief)
        .execute(&mut *writer)
        .await
        .expect("re-point the commission, uncommitted");
    assert_eq!(repointed.rows_affected(), 1, "one commission is re-pointed");
    let fill_pool = pool.clone();
    let fill =
        tokio::spawn(
            async move { try_run_migrations_through(&fill_pool, SLOT_FILL_VERSION).await },
        );

    wait_until_a_lock_on_waits(&pool, "commission").await;
    writer.commit().await.expect("commit the re-point");
    let stopped = fill
        .await
        .expect("join the fill")
        .expect_err("the fill stops");

    assert_stopped_naming(&pool, &stopped, node_less, &[node_less]).await;
}

// ─── The seeded run ────────────────────────────────────────────────────────

/// A database migrated through the node table, seeded with Slots made before it and Slots
/// declared since on the old path, node-less, then migrated up to just before the fill.
/// Answers the commission and every seeded Slot.
pub(crate) async fn seeded_through_den_1() -> (PgPool, impl Sized, Uuid, Vec<Uuid>) {
    let (pool, container) = just_before_den_1().await;
    let [first_before, second_before, first_since, second_since] = SEEDED_SLOT_TITLES;
    let commission = seed_commission(&pool, CANARY_TITLE, "private").await;
    let mut slots = vec![
        seed_slot(&pool, commission, first_before).await,
        seed_slot(&pool, commission, second_before).await,
    ];
    run_migrations_through(&pool, DEN_1_VERSION).await;
    slots.push(seed_slot(&pool, commission, first_since).await);
    slots.push(seed_slot(&pool, commission, second_since).await);
    run_migrations_before(&pool, SLOT_FILL_VERSION).await;
    (pool, container, commission, slots)
}

// Through the real path: a clean run binds every Slot with no phantom left; each plant stops
// the migration naming its id alone, never the canary title.
#[tokio::test]
async fn a_seeded_database_runs_the_fill_through_the_real_path() {
    let (clean, _clean_container, commission, slots) = seeded_through_den_1().await;

    run_migrations_through(&clean, SLOT_FILL_VERSION).await;

    let entry = slots_entry(&clean, commission).await;
    for slot in &slots {
        let node = slot_node(&clean, *slot).await;
        assert_eq!(node.segment, slot.to_string());
        assert_eq!(node.parent, Some(entry));
        assert_eq!(node.level, "private");
    }
    assert_eq!(phantom_count(&clean).await, 0);

    let (phantom_db, _phantom_container, phantom_commission, phantom_slots) =
        seeded_through_den_1().await;
    let phantom = phantom_slot_node(&phantom_db, phantom_commission, CANARY_TITLE).await;
    let (differing_db, _differing_container, _, differing_slots) = seeded_through_den_1().await;
    let differing = differing_slots[0];
    rename_slot_node(&differing_db, differing, "A renamed Slot").await;

    for (label, pool, named, node_less) in [
        (
            "a phantom slot node",
            &phantom_db,
            phantom,
            &phantom_slots[2..],
        ),
        (
            "a differing name",
            &differing_db,
            differing,
            &differing_slots[2..],
        ),
    ] {
        let stopped = try_run_migrations_through(pool, SLOT_FILL_VERSION)
            .await
            .expect_err("the fill stops");
        let everything_it_says = format!("{} {stopped} {stopped:?}", refusal(&stopped));
        assert!(
            !everything_it_says.contains(CANARY_TITLE),
            "{label}: no refusal carries the canary: {everything_it_says}"
        );
        assert_stopped_naming(pool, &stopped, named, node_less).await;
    }
}

// ─── The guards it adds ────────────────────────────────────────────────────

#[tokio::test]
async fn a_node_less_slot_insert_is_refused_after_the_fill() {
    let (pool, _container) = just_before_the_fill().await;
    let commission = bound_commission(&pool, "A commission").await;
    let bound = seed_slot(&pool, commission, "A Slot made before the fill").await;
    run_migrations_through(&pool, SLOT_FILL_VERSION).await;

    let rebound = sqlx::query("UPDATE commission_slot SET node_id = NULL WHERE element_id = $1")
        .bind(bound)
        .execute(&pool)
        .await;
    let element = Uuid::now_v7();
    let inserted = sqlx::query(
        "WITH element AS (
             INSERT INTO commission_element
                 (id, commission_id, tab_id, surface, type, position, created_by, created_at)
             SELECT $1, e.commission_id, e.tab_id, e.surface, 'slot', e.position + 1, e.created_by,
                    now()
             FROM commission_element e WHERE e.id = $2
             RETURNING id, commission_id
         )
         INSERT INTO commission_slot (element_id, commission_id, title)
         SELECT id, commission_id, 'Node-less' FROM element",
    )
    .bind(element)
    .bind(bound)
    .execute(&pool)
    .await;

    assert!(rebound.is_err(), "a Slot keeps its node");
    let error = inserted.expect_err("the database refuses a node-less Slot");
    let refusing = error
        .as_database_error()
        .and_then(|database_error| database_error.constraint());
    assert_eq!(refusing, Some("commission_slot_has_its_node"), "{error:?}");
}

#[tokio::test]
async fn a_slot_node_without_its_slot_is_refused_at_commit_after_the_fill() {
    let (pool, _container) = just_before_the_fill().await;
    let commission = bound_commission(&pool, "A commission").await;
    run_migrations_through(&pool, SLOT_FILL_VERSION).await;
    let parent = slots_entry(&pool, commission).await;

    let inserted = sqlx::query(
        "INSERT INTO node (id, parent, segment, type, level, name)
         VALUES ($1, $2, $3, 'slot', 'private', 'A phantom')",
    )
    .bind(Uuid::now_v7())
    .bind(parent)
    .bind(Uuid::now_v7().to_string())
    .execute(&pool)
    .await;

    let error = inserted.expect_err("the commit refuses a slot node without its slot");
    let message = error
        .as_database_error()
        .map(|database_error| database_error.message().to_owned())
        .unwrap_or_default();
    assert!(message.contains("has no slot"), "{error:?}");
}
