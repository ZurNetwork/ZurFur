//! The commission node fill: every test seeds made-up rows with raw SQL on a
//! database migrated through the node table's migration (some before it, so the
//! node table's own backfill binds them), then runs the fill and nothing after
//! it. Checks the nodes it writes and binds, the titles it re-syncs, the rows it
//! leaves alone, its refusals (each naming an id alone, the fill unrecorded and
//! every node-less commission still without a node), and the guards it adds: a
//! node-less commission row is refused, and a node's rename reaches its
//! commission's title while its level never reaches the visibility. Requires a
//! container runtime socket (DOCKER_HOST honored).

use adapter_pg::PgPool;
use sqlx::migrate::MigrateError;
use uuid::Uuid;

use crate::migration_steps::{
    bare_pool, run_migrations_before, run_migrations_through, try_run_migrations_through,
};
use crate::node_backfill::{
    DEN_1_VERSION, NodeRow, OWNER_DID, archive, every_node, seed_commission, seed_owner,
};

/// The commission node fill's migration, as sqlx numbers it.
pub(crate) const FILL_VERSION: i64 = 20261022132603;

/// The six fixed entry words, as the node table sorts them.
const ENTRY_WORDS: [&str; 6] = [
    "attachments",
    "brief",
    "changelog",
    "participants",
    "products",
    "slots",
];

/// The three level tokens.
pub(crate) const LEVEL_TOKENS: [&str; 3] = ["private", "listed", "public"];

/// A fresh database migrated up to just before the node table, with its owner.
pub(crate) async fn just_before_den_1() -> (PgPool, impl Sized) {
    let (pool, container) = bare_pool().await;
    run_migrations_before(&pool, DEN_1_VERSION).await;
    seed_owner(&pool).await;
    (pool, container)
}

/// A fresh database migrated through the node table's migration, with its owner.
pub(crate) async fn through_den_1() -> (PgPool, impl Sized) {
    let (pool, container) = just_before_den_1().await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    (pool, container)
}

/// The node a commission row points at.
pub(crate) async fn commission_node(pool: &PgPool, commission: Uuid) -> NodeRow {
    sqlx::query_as("SELECT n.* FROM node n JOIN commission c ON c.node_id = n.id WHERE c.id = $1")
        .bind(commission)
        .fetch_one(pool)
        .await
        .expect("read a commission's node")
}

/// The nodes directly under `parent`, by segment.
async fn children(pool: &PgPool, parent: Uuid) -> Vec<NodeRow> {
    sqlx::query_as("SELECT * FROM node WHERE parent = $1 ORDER BY segment")
        .bind(parent)
        .fetch_all(pool)
        .await
        .expect("read a node's children")
}

/// The node id a commission row points at, or `None`.
pub(crate) async fn node_id_of(pool: &PgPool, commission: Uuid) -> Option<Uuid> {
    sqlx::query_scalar("SELECT node_id FROM commission WHERE id = $1")
        .bind(commission)
        .fetch_one(pool)
        .await
        .expect("read a commission's node_id")
}

/// Whether the ledger records `version`.
pub(crate) async fn ledger_records(pool: &PgPool, version: i64) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = $1)")
        .bind(version)
        .fetch_one(pool)
        .await
        .expect("read the ledger")
}

/// Every row of `table` as JSON without `node_id`, ordered by `key`.
async fn rows_without_node_id(pool: &PgPool, table: &'static str, key: &'static str) -> String {
    let rows = format!(
        "SELECT coalesce(jsonb_agg(to_jsonb(t) - 'node_id' ORDER BY t.{key}), '[]')::text FROM {table} t"
    );
    sqlx::query_scalar(sqlx::AssertSqlSafe(rows))
        .fetch_one(pool)
        .await
        .expect("read a table's rows")
}

/// The entries `parent` holds, as (segment, type, name, level).
async fn entries_of(pool: &PgPool, parent: Uuid) -> Vec<(String, String, String, String)> {
    children(pool, parent)
        .await
        .into_iter()
        .map(|entry| (entry.segment, entry.node_type, entry.name, entry.level))
        .collect()
}

/// The six entries every commission node holds, each Private and named by its word.
fn six_private_entries() -> Vec<(String, String, String, String)> {
    ENTRY_WORDS
        .iter()
        .map(|word| {
            (
                (*word).to_owned(),
                format!("commission.{word}"),
                (*word).to_owned(),
                "private".to_owned(),
            )
        })
        .collect()
}

/// Asserts `commission` is bound to a node of its own carrying `title` and `level`, with its
/// six Private entries.
async fn assert_filled(pool: &PgPool, commission: Uuid, title: &str, level: &str) {
    let node = commission_node(pool, commission).await;
    let expected = NodeRow {
        id: node.id,
        parent: None,
        segment: commission.to_string(),
        node_type: "commission".to_owned(),
        level: level.to_owned(),
        name: title.to_owned(),
    };
    assert_eq!(node, expected, "{title} has its node");
    assert_eq!(
        entries_of(pool, node.id).await,
        six_private_entries(),
        "{title} has its six private entries"
    );
}

/// The database's own refusal text inside `stopped`.
pub(crate) fn refusal(stopped: &MigrateError) -> String {
    let (MigrateError::Execute(error) | MigrateError::ExecuteMigration(error, _)) = stopped else {
        panic!("the database refused the migration: {stopped:?}");
    };
    let database_error = error
        .as_database_error()
        .unwrap_or_else(|| panic!("the database refused the migration: {stopped:?}"));
    database_error.message().to_owned()
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
        !ledger_records(pool, FILL_VERSION).await,
        "the ledger does not record the stopped fill"
    );
    for commission in node_less {
        assert_eq!(
            node_id_of(pool, *commission).await,
            None,
            "{commission} still has no node"
        );
    }
}

/// Insert a commission node and the entries named by `words` bare, and point `commission` at
/// it, in one committed unit; the node table's migration checks no entry.
async fn bind_with_entries(pool: &PgPool, commission: Uuid, words: &[&str]) -> Uuid {
    let node = Uuid::now_v7();
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query(
        "INSERT INTO node (id, parent, segment, type, level, name)
         VALUES ($1, NULL, $2, 'commission', 'private', 'A bound commission')",
    )
    .bind(node)
    .bind(commission.to_string())
    .execute(&mut *tx)
    .await
    .expect("insert the commission node");
    for word in words {
        sqlx::query(
            "INSERT INTO node (id, parent, segment, type, level, name)
             VALUES ($1, $2, $3, 'commission.' || $3, 'private', $3)",
        )
        .bind(Uuid::now_v7())
        .bind(node)
        .bind(*word)
        .execute(&mut *tx)
        .await
        .expect("insert an entry");
    }
    sqlx::query("UPDATE commission SET node_id = $2 WHERE id = $1")
        .bind(commission)
        .bind(node)
        .execute(&mut *tx)
        .await
        .expect("point the commission at its node");
    tx.commit().await.expect("the bound commission commits");
    node
}

/// Insert a bare top-level commission node no commission points at, committed.
async fn phantom_node(pool: &PgPool) -> Uuid {
    let node = Uuid::now_v7();
    sqlx::query(
        "INSERT INTO node (id, parent, segment, type, level, name)
         VALUES ($1, NULL, $2, 'commission', 'private', 'A phantom commission')",
    )
    .bind(node)
    .bind(Uuid::now_v7().to_string())
    .execute(pool)
    .await
    .expect("insert a phantom commission node");
    node
}

// ─── The fill ──────────────────────────────────────────────────────────────

#[tokio::test]
async fn a_commission_made_since_den_1_gets_its_node_and_six_entries() {
    let (pool, _container) = through_den_1().await;
    let made = seed_commission(&pool, "A commission made since", "private").await;

    run_migrations_through(&pool, FILL_VERSION).await;

    assert_filled(&pool, made, "A commission made since", "private").await;
}

#[tokio::test]
async fn an_archived_one_too() {
    let (pool, _container) = through_den_1().await;
    let archived = seed_commission(&pool, "An archived commission", "public").await;
    archive(&pool, archived).await;

    run_migrations_through(&pool, FILL_VERSION).await;

    assert_filled(&pool, archived, "An archived commission", "public").await;
}

#[tokio::test]
async fn a_listed_window_commission_keeps_its_level_and_its_entries_are_private() {
    let (pool, _container) = through_den_1().await;
    let listed = seed_commission(&pool, "A listed window commission", "listed").await;

    run_migrations_through(&pool, FILL_VERSION).await;

    assert_filled(&pool, listed, "A listed window commission", "listed").await;
}

#[tokio::test]
async fn a_title_renamed_since_den_1_is_re_synced() {
    let (pool, _container) = just_before_den_1().await;
    let renamed = seed_commission(&pool, "The old title", "private").await;
    let kept = seed_commission(&pool, "A kept title", "public").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    sqlx::query("UPDATE commission SET title = 'The new title' WHERE id = $1")
        .bind(renamed)
        .execute(&pool)
        .await
        .expect("rename on the old path");
    let renamed_node = commission_node(&pool, renamed).await.id;

    run_migrations_through(&pool, FILL_VERSION).await;

    let node = commission_node(&pool, renamed).await;
    assert_eq!(node.id, renamed_node, "the node is the one it had");
    assert_eq!(node.name, "The new title", "the node's name is re-synced");
    assert_eq!(commission_node(&pool, kept).await.name, "A kept title");
    let names: Vec<String> = every_node(&pool)
        .await
        .into_iter()
        .map(|node| node.name)
        .collect();
    assert!(
        !names.iter().any(|name| name == "The old title"),
        "no node keeps the old title: {names:?}"
    );
}

#[tokio::test]
async fn the_fill_changes_no_other_value() {
    let (pool, _container) = just_before_den_1().await;
    let bound = seed_commission(&pool, "A bound commission", "listed").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    let made = seed_commission(&pool, "A commission made since", "public").await;
    archive(&pool, made).await;
    let commissions_before = rows_without_node_id(&pool, "commission", "id").await;
    let nodes_before = every_node(&pool).await;
    let bound_node = node_id_of(&pool, bound).await;

    run_migrations_through(&pool, FILL_VERSION).await;

    assert_eq!(
        rows_without_node_id(&pool, "commission", "id").await,
        commissions_before,
        "no commission value changes but the node id it lacked"
    );
    assert_eq!(node_id_of(&pool, bound).await, bound_node);
    let nodes_after = every_node(&pool).await;
    let kept: Vec<&NodeRow> = nodes_after
        .iter()
        .filter(|node| nodes_before.contains(node))
        .collect();
    assert_eq!(kept.len(), nodes_before.len(), "every earlier node stands");
    assert_eq!(
        nodes_after.len(),
        nodes_before.len() + 7,
        "the made commission's node and its six entries are all that is added"
    );
}

#[tokio::test]
async fn a_database_with_no_commissions_migrates() {
    let (pool, _container) = bare_pool().await;

    run_migrations_through(&pool, FILL_VERSION).await;

    assert!(every_node(&pool).await.is_empty());
    assert!(ledger_records(&pool, FILL_VERSION).await);
}

// ─── The guards it adds ────────────────────────────────────────────────────

#[tokio::test]
async fn a_node_less_commission_insert_is_refused_after_the_fill() {
    let (pool, _container) = through_den_1().await;
    run_migrations_through(&pool, FILL_VERSION).await;

    let inserted = sqlx::query(
        "INSERT INTO commission (id, title, owner_id, lifecycle, visibility, created_at)
         VALUES ($1, 'A node-less commission', $2, 'draft', 'private', now())",
    )
    .bind(Uuid::now_v7())
    .bind(OWNER_DID)
    .execute(&pool)
    .await;

    let error = inserted.expect_err("the database refuses");
    let refusing = error
        .as_database_error()
        .and_then(|database_error| database_error.constraint());
    assert_eq!(refusing, Some("commission_has_its_node"), "{error:?}");
}

#[tokio::test]
async fn a_node_rename_reaches_the_title_and_a_widening_leaves_the_visibility() {
    let (pool, _container) = through_den_1().await;
    let commission = seed_commission(&pool, "Before", "private").await;
    run_migrations_through(&pool, FILL_VERSION).await;
    let node = commission_node(&pool, commission).await.id;

    sqlx::query("UPDATE node SET name = 'After', level = 'listed' WHERE id = $1")
        .bind(node)
        .execute(&pool)
        .await
        .expect("rename and widen the node");

    let (title, visibility): (String, String) =
        sqlx::query_as("SELECT title, visibility FROM commission WHERE id = $1")
            .bind(commission)
            .fetch_one(&pool)
            .await
            .expect("read the commission's own columns");
    assert_eq!(title, "After", "the title follows the node's name");
    assert_eq!(
        visibility, "private",
        "the visibility keeps its value: no trigger copies the node's level"
    );
}

// ─── Its refusals ──────────────────────────────────────────────────────────

#[tokio::test]
async fn a_level_that_differs_stops_the_fill_naming_the_commission() {
    let (pool, _container) = just_before_den_1().await;
    let differing = seed_commission(&pool, "A widened commission", "private").await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    let node_less = seed_commission(&pool, "A commission made since", "private").await;
    sqlx::query("UPDATE commission SET visibility = 'public' WHERE id = $1")
        .bind(differing)
        .execute(&pool)
        .await
        .expect("set the visibility apart from the node's level");

    let stopped = try_run_migrations_through(&pool, FILL_VERSION)
        .await
        .expect_err("the fill stops");

    assert_stopped_naming(&pool, &stopped, differing, &[node_less]).await;
    let level: String = sqlx::query_scalar(
        "SELECT n.level FROM node n JOIN commission c ON c.node_id = n.id WHERE c.id = $1",
    )
    .bind(differing)
    .fetch_one(&pool)
    .await
    .expect("read the node's level");
    assert_eq!(level, "private", "no level is re-synced");
}

#[tokio::test]
async fn a_phantom_commission_node_stops_the_fill_naming_the_node() {
    let (pool, _container) = through_den_1().await;
    let node_less = seed_commission(&pool, "A commission made since", "private").await;
    let phantom = phantom_node(&pool).await;

    let stopped = try_run_migrations_through(&pool, FILL_VERSION)
        .await
        .expect_err("the fill stops");

    assert_stopped_naming(&pool, &stopped, phantom, &[node_less]).await;
}

#[tokio::test]
async fn an_empty_title_stops_the_fill_naming_the_commission() {
    let (pool, _container) = through_den_1().await;
    let node_less = seed_commission(&pool, "A titled commission", "private").await;
    let untitled = seed_commission(&pool, "", "private").await;

    let stopped = try_run_migrations_through(&pool, FILL_VERSION)
        .await
        .expect_err("the fill stops");

    assert_stopped_naming(&pool, &stopped, untitled, &[node_less, untitled]).await;
}

#[tokio::test]
async fn an_unreadable_visibility_stops_it() {
    let (pool, _container) = through_den_1().await;
    let node_less = seed_commission(&pool, "A readable commission", "private").await;
    let unreadable = seed_commission(&pool, "An unreadable commission", "Private").await;

    let stopped = try_run_migrations_through(&pool, FILL_VERSION)
        .await
        .expect_err("the fill stops");

    assert_stopped_naming(&pool, &stopped, unreadable, &[node_less, unreadable]).await;
}

#[tokio::test]
async fn a_commission_node_missing_an_entry_stops_it() {
    let (pool, _container) = through_den_1().await;
    let node_less = seed_commission(&pool, "A commission made since", "private").await;
    let short = seed_commission(&pool, "A bound commission", "private").await;
    let node = bind_with_entries(&pool, short, &ENTRY_WORDS[..5]).await;

    let stopped = try_run_migrations_through(&pool, FILL_VERSION)
        .await
        .expect_err("the fill stops");

    assert_stopped_naming(&pool, &stopped, node, &[node_less]).await;
}

#[tokio::test]
async fn the_refusals_name_neither_token_nor_title() {
    let title = "A distinctive commission title";
    let unreadable_token = "secret-visibility-token";

    let differing = {
        let (pool, container) = just_before_den_1().await;
        let commission = seed_commission(&pool, title, "listed").await;
        run_migrations_through(&pool, DEN_1_VERSION).await;
        sqlx::query("UPDATE commission SET visibility = 'public' WHERE id = $1")
            .bind(commission)
            .execute(&pool)
            .await
            .expect("set the visibility apart");
        (pool, container)
    };
    let unreadable = {
        let (pool, container) = just_before_den_1().await;
        run_migrations_through(&pool, DEN_1_VERSION).await;
        seed_commission(&pool, title, unreadable_token).await;
        (pool, container)
    };
    let phantom = {
        let (pool, container) = just_before_den_1().await;
        run_migrations_through(&pool, DEN_1_VERSION).await;
        seed_commission(&pool, title, "public").await;
        phantom_node(&pool).await;
        (pool, container)
    };
    let short = {
        let (pool, container) = just_before_den_1().await;
        run_migrations_through(&pool, DEN_1_VERSION).await;
        let commission = seed_commission(&pool, title, "private").await;
        bind_with_entries(&pool, commission, &ENTRY_WORDS[1..]).await;
        (pool, container)
    };

    for (label, (pool, _container)) in [
        ("a differing level", differing),
        ("an unreadable visibility", unreadable),
        ("a phantom node", phantom),
        ("a missing entry", short),
    ] {
        let stopped = try_run_migrations_through(&pool, FILL_VERSION)
            .await
            .expect_err("the fill stops");
        let everything_it_says = format!("{stopped} {stopped:?}");
        assert!(
            !everything_it_says.contains(title),
            "{label}: the refusal never echoes the title: {everything_it_says}"
        );
        assert!(
            !everything_it_says.contains(unreadable_token),
            "{label}: the refusal never echoes the token: {everything_it_says}"
        );
        for token in LEVEL_TOKENS {
            assert!(
                !everything_it_says.contains(token),
                "{label}: the refusal never names a level token: {everything_it_says}"
            );
        }
        assert!(
            !ledger_records(&pool, FILL_VERSION).await,
            "{label}: the ledger does not record the stopped fill"
        );
    }
}
