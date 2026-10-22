//! The node table's migration and backfill: every test runs the migrations
//! before it, seeds made-up commissions and Slots with raw SQL at that schema,
//! then runs the migration and nothing after it. Checks the nodes it writes,
//! the rows it leaves alone, the two refusals, and that a deleted commission or
//! Slot takes its nodes. Requires a container runtime socket (DOCKER_HOST honored).

use adapter_pg::PgPool;
use uuid::Uuid;

use crate::migration_steps::{
    bare_pool, run_migrations_before, run_migrations_through, try_run_migrations_through,
};

/// The node table's migration, as sqlx numbers it.
pub const DEN_1_VERSION: i64 = 20261022052438;

/// The made-up owner of every seeded commission.
pub const OWNER_DID: &str = "did:plc:n0deownerfixtureaaaaaaaa";

/// One node as the table stores it.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct NodeRow {
    pub id: Uuid,
    pub parent: Option<Uuid>,
    pub segment: String,
    #[sqlx(rename = "type")]
    pub node_type: String,
    pub level: String,
    pub name: String,
}

/// Seed the owner: an actor identity and its User.
pub async fn seed_owner(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO actor_identity (id, kind, did, state, first_seen)
         VALUES ($1, 'user', $2, 'active', now())",
    )
    .bind(Uuid::now_v7())
    .bind(OWNER_DID)
    .execute(pool)
    .await
    .expect("seed the owner's identity");
    sqlx::query("INSERT INTO users (id, created_at) VALUES ($1, now())")
        .bind(OWNER_DID)
        .execute(pool)
        .await
        .expect("seed the owner");
}

/// Seed one commission row, with no node, and answer its id.
pub async fn seed_commission(pool: &PgPool, title: &str, visibility: &str) -> Uuid {
    let id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO commission (id, title, owner_id, lifecycle, visibility, created_at)
         VALUES ($1, $2, $3, 'draft', $4, now())",
    )
    .bind(id)
    .bind(title)
    .bind(OWNER_DID)
    .bind(visibility)
    .execute(pool)
    .await
    .expect("seed a commission");
    id
}

/// Mark a seeded commission archived.
pub async fn archive(pool: &PgPool, commission: Uuid) {
    sqlx::query("UPDATE commission SET archived_at = now() WHERE id = $1")
        .bind(commission)
        .execute(pool)
        .await
        .expect("archive a commission");
}

/// Seed one Slot, with no node, on the commission's `main` tab (made on
/// first use), and answer its element id.
pub async fn seed_slot(pool: &PgPool, commission: Uuid, title: &str) -> Uuid {
    sqlx::query(
        "INSERT INTO commission_tab (id, commission_id, tab) VALUES ($1, $2, 'main')
         ON CONFLICT (commission_id, tab) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(commission)
    .execute(pool)
    .await
    .expect("seed the main tab");
    let element_id = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO commission_element
             (id, commission_id, tab_id, surface, type, position, created_by, created_at)
         SELECT $1, $2, t.id, 'characters', 'slot',
                (SELECT count(*) FROM commission_element e WHERE e.commission_id = $2),
                $3, now()
         FROM commission_tab t
         WHERE t.commission_id = $2 AND t.tab = 'main'",
    )
    .bind(element_id)
    .bind(commission)
    .bind(OWNER_DID)
    .execute(pool)
    .await
    .expect("seed a slot's element");
    sqlx::query(
        "INSERT INTO commission_slot (element_id, commission_id, title) VALUES ($1, $2, $3)",
    )
    .bind(element_id)
    .bind(commission)
    .bind(title)
    .execute(pool)
    .await
    .expect("seed a slot");
    element_id
}

/// Every node, in a stable order.
pub async fn every_node(pool: &PgPool) -> Vec<NodeRow> {
    sqlx::query_as("SELECT * FROM node ORDER BY type COLLATE \"C\", segment, id")
        .fetch_all(pool)
        .await
        .expect("read every node")
}

/// The node a commission row points at.
async fn commission_node(pool: &PgPool, commission: Uuid) -> NodeRow {
    sqlx::query_as("SELECT n.* FROM node n JOIN commission c ON c.node_id = n.id WHERE c.id = $1")
        .bind(commission)
        .fetch_one(pool)
        .await
        .expect("read a commission's node")
}

/// The node a Slot row points at.
async fn slot_node(pool: &PgPool, element_id: Uuid) -> NodeRow {
    sqlx::query_as(
        "SELECT n.* FROM node n JOIN commission_slot s ON s.node_id = n.id WHERE s.element_id = $1",
    )
    .bind(element_id)
    .fetch_one(pool)
    .await
    .expect("read a slot's node")
}

/// The nodes directly under `parent`, by segment.
async fn children(pool: &PgPool, parent: Uuid) -> Vec<NodeRow> {
    sqlx::query_as("SELECT * FROM node WHERE parent = $1 ORDER BY segment")
        .bind(parent)
        .fetch_all(pool)
        .await
        .expect("read a node's children")
}

/// A fresh database migrated up to just before the node table, with its owner.
async fn just_before_den_1() -> (PgPool, impl Sized) {
    let (pool, container) = bare_pool().await;
    run_migrations_before(&pool, DEN_1_VERSION).await;
    seed_owner(&pool).await;
    (pool, container)
}

/// Every name a node carries.
async fn node_names(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT name FROM node ORDER BY name COLLATE \"C\"")
        .fetch_all(pool)
        .await
        .expect("read every node's name")
}

/// Whether the ledger records `version`.
async fn ledger_records(pool: &PgPool, version: i64) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = $1)")
        .bind(version)
        .fetch_one(pool)
        .await
        .expect("read the ledger")
}

/// Whether `commission` has a `node_id` column.
async fn commission_has_node_id(pool: &PgPool) -> bool {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM information_schema.columns
                        WHERE table_name = 'commission' AND column_name = 'node_id')",
    )
    .fetch_one(pool)
    .await
    .expect("read the catalog")
}

#[tokio::test]
async fn deleting_a_commission_removes_its_nodes() {
    let (pool, _container) = just_before_den_1().await;
    let doomed = seed_commission(&pool, "A commission to delete", "public").await;
    seed_slot(&pool, doomed, "A first slot to delete").await;
    seed_slot(&pool, doomed, "A second slot to delete").await;
    let kept = seed_commission(&pool, "A commission that stays", "private").await;
    seed_slot(&pool, kept, "A slot that stays").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;

    sqlx::query("DELETE FROM commission WHERE id = $1")
        .bind(doomed)
        .execute(&pool)
        .await
        .expect("the delete commits");

    let names = node_names(&pool).await;
    let expected_names = [
        "A commission that stays",
        "A slot that stays",
        "attachments",
        "brief",
        "changelog",
        "participants",
        "products",
        "slots",
    ];
    assert_eq!(
        names, expected_names,
        "only the kept commission's nodes remain, and no node carries a deleted title"
    );
}

#[tokio::test]
async fn removing_a_slot_s_element_removes_its_node() {
    let (pool, _container) = just_before_den_1().await;
    let commission = seed_commission(&pool, "A commission", "private").await;
    let removed = seed_slot(&pool, commission, "A slot to remove").await;
    seed_slot(&pool, commission, "A slot that stays").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;

    sqlx::query("DELETE FROM commission_element WHERE id = $1 AND commission_id = $2")
        .bind(removed)
        .bind(commission)
        .execute(&pool)
        .await
        .expect("the removal commits");

    let names = node_names(&pool).await;
    let expected_names = [
        "A commission",
        "A slot that stays",
        "attachments",
        "brief",
        "changelog",
        "participants",
        "products",
        "slots",
    ];
    assert_eq!(
        names, expected_names,
        "the removed slot's node is gone, and no node carries its title"
    );
}

#[tokio::test]
async fn hard_deleting_a_commission_without_a_node_succeeds() {
    let (pool, _container) = just_before_den_1().await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    let commission =
        seed_commission(&pool, "A commission made after the backfill", "private").await;
    seed_slot(&pool, commission, "A slot made after the backfill").await;

    let deleted = sqlx::query("DELETE FROM commission WHERE id = $1")
        .bind(commission)
        .execute(&pool)
        .await;

    assert!(
        deleted.is_ok(),
        "a commission with no node deletes: {deleted:?}"
    );
}

#[tokio::test]
async fn removing_the_element_of_a_slot_without_a_node_succeeds() {
    let (pool, _container) = just_before_den_1().await;
    let commission = seed_commission(&pool, "A commission", "private").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    let slot = seed_slot(&pool, commission, "A slot made after the backfill").await;

    let removed =
        sqlx::query("DELETE FROM commission_element WHERE id = $1 AND commission_id = $2")
            .bind(slot)
            .bind(commission)
            .execute(&pool)
            .await;

    assert!(
        removed.is_ok(),
        "a slot with no node is removed: {removed:?}"
    );
}

#[tokio::test]
async fn a_rename_on_today_s_path_after_the_backfill_commits() {
    let (pool, _container) = just_before_den_1().await;
    let backfilled = seed_commission(&pool, "A backfilled commission", "private").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    let unbound = seed_commission(&pool, "A commission made after the backfill", "private").await;

    for (commission, title) in [
        (backfilled, "A renamed backfilled commission"),
        (unbound, "A renamed later commission"),
    ] {
        let renamed = sqlx::query(
            "UPDATE commission SET title = $2 WHERE id = $1 AND title IS DISTINCT FROM $2",
        )
        .bind(commission)
        .bind(title)
        .execute(&pool)
        .await;
        assert!(renamed.is_ok(), "the rename commits: {renamed:?}");
    }
}

#[tokio::test]
async fn every_commission_gets_one_node_with_its_title_and_level() {
    let (pool, _container) = just_before_den_1().await;
    let private = seed_commission(&pool, "A private commission", "private").await;
    let listed = seed_commission(&pool, "A listed commission", "listed").await;
    let public = seed_commission(&pool, "A public commission", "public").await;
    let archived = seed_commission(&pool, "An archived commission", "listed").await;
    archive(&pool, archived).await;
    run_migrations_through(&pool, DEN_1_VERSION).await;

    let seeded = [
        (private, "A private commission", "private"),
        (listed, "A listed commission", "listed"),
        (public, "A public commission", "public"),
        (archived, "An archived commission", "listed"),
    ];
    for (commission, title, level) in seeded {
        let node = commission_node(&pool, commission).await;
        let expected = NodeRow {
            id: node.id,
            parent: None,
            segment: commission.to_string(),
            node_type: "commission".to_owned(),
            level: level.to_owned(),
            name: title.to_owned(),
        };
        assert_eq!(node, expected, "{title} has its node");
    }
    let commission_nodes: i64 =
        sqlx::query_scalar("SELECT count(*) FROM node WHERE type = 'commission'")
            .fetch_one(&pool)
            .await
            .expect("count commission nodes");
    assert_eq!(commission_nodes, 4, "one node per commission");
}

#[tokio::test]
async fn no_entry_or_slot_is_wider_than_private() {
    let (pool, _container) = just_before_den_1().await;
    let commission = seed_commission(&pool, "A public commission", "public").await;
    seed_slot(&pool, commission, "A first slot").await;
    seed_slot(&pool, commission, "A second slot").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;

    let levels: Vec<(String, String)> = sqlx::query_as(
        "SELECT type, level FROM node WHERE type <> 'commission' ORDER BY type COLLATE \"C\"",
    )
    .fetch_all(&pool)
    .await
    .expect("read the levels");
    let expected_levels = [
        ("commission.attachments", "private"),
        ("commission.brief", "private"),
        ("commission.changelog", "private"),
        ("commission.participants", "private"),
        ("commission.products", "private"),
        ("commission.slots", "private"),
        ("slot", "private"),
        ("slot", "private"),
    ]
    .map(|(node_type, level)| (node_type.to_owned(), level.to_owned()));
    assert_eq!(levels, expected_levels);
}

#[tokio::test]
async fn each_commission_gets_its_six_entries_private() {
    let (pool, _container) = just_before_den_1().await;
    let first = seed_commission(&pool, "A first commission", "public").await;
    let second = seed_commission(&pool, "A second commission", "listed").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;

    for commission in [first, second] {
        let node = commission_node(&pool, commission).await;
        let entries: Vec<(String, String, String, String)> = children(&pool, node.id)
            .await
            .into_iter()
            .map(|entry| (entry.segment, entry.node_type, entry.name, entry.level))
            .collect();
        let expected_entries = [
            (
                "attachments",
                "commission.attachments",
                "attachments",
                "private",
            ),
            ("brief", "commission.brief", "brief", "private"),
            ("changelog", "commission.changelog", "changelog", "private"),
            (
                "participants",
                "commission.participants",
                "participants",
                "private",
            ),
            ("products", "commission.products", "products", "private"),
            ("slots", "commission.slots", "slots", "private"),
        ]
        .map(|(segment, node_type, name, level)| {
            (
                segment.to_owned(),
                node_type.to_owned(),
                name.to_owned(),
                level.to_owned(),
            )
        });
        assert_eq!(
            entries, expected_entries,
            "commission {commission}'s entries"
        );
    }
}

#[tokio::test]
async fn each_slot_sits_under_its_own_commission_s_slots_entry() {
    let (pool, _container) = just_before_den_1().await;
    let first = seed_commission(&pool, "A first commission", "private").await;
    let second = seed_commission(&pool, "A second commission", "private").await;
    let first_slot = seed_slot(&pool, first, "A slot of the first").await;
    let second_slot = seed_slot(&pool, second, "A slot of the second").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;

    for (commission, slot) in [(first, first_slot), (second, second_slot)] {
        let commission_node = commission_node(&pool, commission).await;
        let slot_node = slot_node(&pool, slot).await;
        let slots_entry: NodeRow = sqlx::query_as("SELECT * FROM node WHERE id = $1")
            .bind(slot_node.parent)
            .fetch_one(&pool)
            .await
            .expect("read the slot's parent");
        assert_eq!(slots_entry.node_type, "commission.slots");
        assert_eq!(
            slots_entry.parent,
            Some(commission_node.id),
            "slot {slot} sits under its own commission's slots entry"
        );
    }
}

#[tokio::test]
async fn the_commission_and_slot_rows_point_at_their_nodes() {
    let (pool, _container) = just_before_den_1().await;
    let first = seed_commission(&pool, "A first commission", "private").await;
    let second = seed_commission(&pool, "A second commission", "public").await;
    seed_slot(&pool, first, "A first slot").await;
    seed_slot(&pool, second, "A second slot").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;

    let pointing: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT 'commission', n.type FROM commission c LEFT JOIN node n ON n.id = c.node_id
         UNION ALL
         SELECT 'slot', n.type FROM commission_slot s LEFT JOIN node n ON n.id = s.node_id
         ORDER BY 1, 2",
    )
    .fetch_all(&pool)
    .await
    .expect("read what each row points at");
    let expected_pointing = [
        ("commission", "commission"),
        ("commission", "commission"),
        ("slot", "slot"),
        ("slot", "slot"),
    ]
    .map(|(row, node_type)| (row.to_owned(), Some(node_type.to_owned())));
    assert_eq!(pointing, expected_pointing);
}

#[tokio::test]
async fn a_commission_s_segment_is_its_id_text() {
    let (pool, _container) = just_before_den_1().await;
    let commission = seed_commission(&pool, "A commission", "private").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;

    let node = commission_node(&pool, commission).await;

    assert_eq!(node.segment, commission.to_string());
}

#[tokio::test]
async fn a_slot_s_segment_is_its_element_id_text() {
    let (pool, _container) = just_before_den_1().await;
    let commission = seed_commission(&pool, "A commission", "private").await;
    let slot = seed_slot(&pool, commission, "A slot").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;

    let node = slot_node(&pool, slot).await;

    assert_eq!(node.segment, slot.to_string());
}

#[tokio::test]
async fn a_title_the_name_rule_refuses_is_copied_verbatim() {
    let (pool, _container) = just_before_den_1().await;
    let commission_title = "A commission \u{202E}desrever";
    let slot_title = "  A slot\twith a tab  ";
    let commission = seed_commission(&pool, commission_title, "private").await;
    let slot = seed_slot(&pool, commission, slot_title).await;
    run_migrations_through(&pool, DEN_1_VERSION).await;

    let commission_node = commission_node(&pool, commission).await;
    let slot_node = slot_node(&pool, slot).await;

    assert_eq!(commission_node.name, commission_title);
    assert_eq!(slot_node.name, slot_title);
}

/// Every row of `table` as JSON without its `node_id`, ordered by `key`.
async fn rows_without_node_id(pool: &PgPool, table: &'static str, key: &'static str) -> String {
    let rows = format!(
        "SELECT coalesce(jsonb_agg(to_jsonb(t) - 'node_id' ORDER BY t.{key}), '[]')::text FROM {table} t"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(rows))
        .fetch_one(pool)
        .await
        .expect("read a table's rows")
}

#[tokio::test]
async fn the_backfill_changes_no_existing_value() {
    let (pool, _container) = just_before_den_1().await;
    let first = seed_commission(&pool, "A first commission", "public").await;
    let second = seed_commission(&pool, "A second commission", "listed").await;
    archive(&pool, second).await;
    seed_slot(&pool, first, "A first slot").await;
    seed_slot(&pool, second, "A second slot").await;
    let commissions_before = rows_without_node_id(&pool, "commission", "id").await;
    let slots_before = rows_without_node_id(&pool, "commission_slot", "element_id").await;

    run_migrations_through(&pool, DEN_1_VERSION).await;

    let commissions_after = rows_without_node_id(&pool, "commission", "id").await;
    let slots_after = rows_without_node_id(&pool, "commission_slot", "element_id").await;
    assert_eq!(commissions_after, commissions_before);
    assert_eq!(slots_after, slots_before);
}

#[tokio::test]
async fn a_database_with_no_commissions_migrates() {
    let (pool, _container) = just_before_den_1().await;

    run_migrations_through(&pool, DEN_1_VERSION).await;

    assert!(every_node(&pool).await.is_empty());
}

#[tokio::test]
async fn the_backfill_writes_these_types() {
    let (pool, _container) = just_before_den_1().await;
    let first = seed_commission(&pool, "A first commission", "private").await;
    let second = seed_commission(&pool, "A second commission", "public").await;
    seed_slot(&pool, first, "A first slot").await;
    seed_slot(&pool, first, "A second slot").await;
    seed_slot(&pool, second, "A third slot").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;

    let counts: Vec<(String, i64)> =
        sqlx::query_as("SELECT type, count(*) FROM node GROUP BY type ORDER BY type COLLATE \"C\"")
            .fetch_all(&pool)
            .await
            .expect("count the types");
    let expected_counts = [
        ("commission", 2),
        ("commission.attachments", 2),
        ("commission.brief", 2),
        ("commission.changelog", 2),
        ("commission.participants", 2),
        ("commission.products", 2),
        ("commission.slots", 2),
        ("slot", 3),
    ]
    .map(|(node_type, count)| (node_type.to_owned(), count));
    assert_eq!(counts, expected_counts);
}

/// Asserts the migration left nothing behind: no ledger row, no column.
async fn assert_nothing_was_written(pool: &PgPool) {
    assert!(
        !ledger_records(pool, DEN_1_VERSION).await,
        "the ledger does not record the stopped migration"
    );
    assert!(
        !commission_has_node_id(pool).await,
        "commission has no node_id column"
    );
}

#[tokio::test]
async fn an_unreadable_visibility_stops_the_migration_naming_the_commission() {
    let (pool, _container) = just_before_den_1().await;
    seed_commission(&pool, "A readable commission", "private").await;
    let unreadable = seed_commission(&pool, "An unreadable commission", "Private").await;

    let stopped = try_run_migrations_through(&pool, DEN_1_VERSION)
        .await
        .expect_err("the migration stops");

    let message = stopped.to_string();
    assert!(
        message.contains(&unreadable.to_string()),
        "the refusal names the commission: {message}"
    );
    assert!(
        message.contains("visibility"),
        "the refusal names the column: {message}"
    );
    assert_nothing_was_written(&pool).await;
}

#[tokio::test]
async fn an_empty_title_stops_the_migration_naming_the_commission() {
    let (pool, _container) = just_before_den_1().await;
    seed_commission(&pool, "A titled commission", "private").await;
    let untitled = seed_commission(&pool, "", "public").await;

    let stopped = try_run_migrations_through(&pool, DEN_1_VERSION)
        .await
        .expect_err("the migration stops");

    let message = stopped.to_string();
    assert!(
        message.contains(&untitled.to_string()),
        "the refusal names the commission: {message}"
    );
    assert!(
        message.contains("title"),
        "the refusal names the column: {message}"
    );
    assert_nothing_was_written(&pool).await;
}

#[tokio::test]
async fn the_refusal_names_neither_the_token_nor_the_title() {
    let (pool, _container) = just_before_den_1().await;
    let token = "secret-visibility-token";
    let title = "A distinctive commission title";
    seed_commission(&pool, title, token).await;

    let stopped = try_run_migrations_through(&pool, DEN_1_VERSION)
        .await
        .expect_err("the migration stops");

    let everything_it_says = format!("{stopped} {stopped:?}");
    assert!(
        !everything_it_says.contains(token),
        "the refusal never echoes the token: {everything_it_says}"
    );
    assert!(
        !everything_it_says.contains(title),
        "the refusal never echoes the title: {everything_it_says}"
    );
    assert_nothing_was_written(&pool).await;
}
