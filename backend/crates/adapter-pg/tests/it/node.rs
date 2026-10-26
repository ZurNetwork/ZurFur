//! The node table's guards, on a fully migrated database: the shape a node
//! takes on its own, what may change after it is written, and how a commission
//! or Slot row and its node stay bound. Raw SQL. A commission node needs its
//! commission at commit, and a slot node its Slot, so a bare one lives only inside an
//! uncommitted unit, and a Slot row is inserted with its node in one unit;
//! the tests of binding a node-less commission row run on a database migrated
//! only through the node table's migration. Requires a container runtime socket
//! (DOCKER_HOST honored).

use std::fmt::Debug;

use adapter_pg::PgPool;
use sqlx::postgres::PgQueryResult;
use uuid::Uuid;

use crate::migration_steps::{bare_pool, run_migrations_through};
use crate::node_backfill::{DEN_1_VERSION, seed_commission, seed_owner};

/// The six fixed entries every commission node holds.
const ENTRY_WORDS: [&str; 6] = [
    "attachments",
    "brief",
    "changelog",
    "participants",
    "products",
    "slots",
];

/// A fresh, fully migrated database with the made-up owner seeded.
async fn migrated() -> (PgPool, impl Sized) {
    let (pool, db) = test_support::pg::fresh_pool().await;
    seed_owner(&pool).await;
    (pool, db)
}

/// A fresh database migrated only through the node table's migration, where a
/// commission row may still lack a node, with the made-up owner seeded.
async fn migrated_through_den_1() -> (PgPool, impl Sized) {
    let (pool, db) = bare_pool().await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    seed_owner(&pool).await;
    (pool, db)
}

/// One node to insert, column for column.
struct NewNode {
    id: Uuid,
    parent: Option<Uuid>,
    segment: String,
    node_type: String,
    level: String,
    name: String,
}

impl NewNode {
    /// A top-level commission node with this segment.
    fn commission(segment: &str) -> Self {
        Self {
            id: Uuid::now_v7(),
            parent: None,
            segment: segment.to_owned(),
            node_type: "commission".to_owned(),
            level: "private".to_owned(),
            name: "A commission".to_owned(),
        }
    }

    /// The fixed entry `word` under `parent`.
    fn entry(parent: Uuid, word: &str) -> Self {
        Self {
            id: Uuid::now_v7(),
            parent: Some(parent),
            segment: word.to_owned(),
            node_type: format!("commission.{word}"),
            level: "private".to_owned(),
            name: word.to_owned(),
        }
    }

    /// A Slot node with this segment under `parent`.
    fn slot(parent: Uuid, segment: &str) -> Self {
        Self {
            id: Uuid::now_v7(),
            parent: Some(parent),
            segment: segment.to_owned(),
            node_type: "slot".to_owned(),
            level: "private".to_owned(),
            name: "A slot".to_owned(),
        }
    }

    /// Insert the node.
    async fn insert(
        &self,
        executor: impl sqlx::PgExecutor<'_>,
    ) -> Result<PgQueryResult, sqlx::Error> {
        sqlx::query(
            "INSERT INTO node (id, parent, segment, type, level, name)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(self.id)
        .bind(self.parent)
        .bind(&self.segment)
        .bind(&self.node_type)
        .bind(&self.level)
        .bind(&self.name)
        .execute(executor)
        .await
    }
}

/// A commission node and its six entries, inserted bare: no row points at it, so
/// on a fully migrated database it commits only once a commission points at it.
struct BareCommissionNode {
    node: Uuid,
    brief: Uuid,
    slots: Uuid,
}

/// Insert a bare commission node with this segment, and its six entries.
async fn bare_commission_node(
    executor: &mut sqlx::PgConnection,
    segment: &str,
) -> BareCommissionNode {
    let commission = NewNode::commission(segment);
    commission
        .insert(&mut *executor)
        .await
        .expect("insert a commission node");
    let entries = ENTRY_WORDS.map(|word| NewNode::entry(commission.id, word));
    for entry in &entries {
        entry.insert(&mut *executor).await.expect("insert an entry");
    }
    let entry_id = |word: &str| {
        entries
            .iter()
            .find(|entry| entry.segment == word)
            .map(|entry| entry.id)
            .expect("one of the six entries")
    };
    BareCommissionNode {
        node: commission.id,
        brief: entry_id("brief"),
        slots: entry_id("slots"),
    }
}

/// A commission row and the nodes it points at.
struct BoundCommission {
    id: Uuid,
    nodes: BareCommissionNode,
}

/// Give an existing commission row its node and entries, and commit.
async fn give_it_its_nodes(pool: &PgPool, commission: Uuid) -> BoundCommission {
    let mut tx = pool.begin().await.expect("begin");
    let nodes = bare_commission_node(&mut tx, &commission.to_string()).await;
    sqlx::query("UPDATE commission SET node_id = $2 WHERE id = $1")
        .bind(commission)
        .bind(nodes.node)
        .execute(&mut *tx)
        .await
        .expect("point the commission at its node");
    tx.commit().await.expect("the bound commission commits");
    BoundCommission {
        id: commission,
        nodes,
    }
}

/// A new commission row inserted with its own nodes in one statement, committed.
async fn bound_commission(pool: &PgPool) -> BoundCommission {
    let id = Uuid::now_v7();
    let mut tx = pool.begin().await.expect("begin");
    let nodes = bare_commission_node(&mut tx, &id.to_string()).await;
    sqlx::query(
        "INSERT INTO commission (id, owner_id, lifecycle, created_at, node_id)
         VALUES ($1, $2, 'draft', now(), $3)",
    )
    .bind(id)
    .bind(crate::node_backfill::OWNER_DID)
    .bind(nodes.node)
    .execute(&mut *tx)
    .await
    .expect("insert the commission bound to its node");
    tx.commit().await.expect("the bound commission commits");
    BoundCommission { id, nodes }
}

/// A Slot row and the node it points at.
struct BoundSlot {
    element_id: Uuid,
    node: Uuid,
}

/// Insert a Slot's carrying element in `commission`'s `main` tab, the tab made first when it
/// has none. Answers the element's id.
async fn slot_element(executor: &mut sqlx::PgConnection, commission: Uuid) -> Uuid {
    sqlx::query(
        "INSERT INTO commission_tab (id, commission_id, tab) VALUES ($1, $2, 'main')
         ON CONFLICT (commission_id, tab) DO NOTHING",
    )
    .bind(Uuid::now_v7())
    .bind(commission)
    .execute(&mut *executor)
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
    .bind(crate::node_backfill::OWNER_DID)
    .execute(&mut *executor)
    .await
    .expect("seed a slot's element");
    element_id
}

/// Insert the Slot row of `element` in `commission`, pointing at `node`.
async fn slot_row_at(
    executor: &mut sqlx::PgConnection,
    element: Uuid,
    commission: Uuid,
    node: Uuid,
) -> Result<PgQueryResult, sqlx::Error> {
    sqlx::query(
        "INSERT INTO commission_slot (element_id, commission_id, title, node_id)
         VALUES ($1, $2, 'A slot', $3)",
    )
    .bind(element)
    .bind(commission)
    .bind(node)
    .execute(&mut *executor)
    .await
}

/// A new Slot of `commission` bound to its own node, its element, node and row in one unit,
/// committed.
async fn bound_slot(pool: &PgPool, commission: &BoundCommission) -> BoundSlot {
    let mut tx = pool.begin().await.expect("begin");
    let element_id = slot_element(&mut tx, commission.id).await;
    let node = NewNode::slot(commission.nodes.slots, &element_id.to_string());
    node.insert(&mut *tx).await.expect("insert the slot's node");
    slot_row_at(&mut tx, element_id, commission.id, node.id)
        .await
        .expect("insert the slot at its node");
    tx.commit().await.expect("the bound slot commits");
    BoundSlot {
        element_id,
        node: node.id,
    }
}

/// How many nodes there are.
async fn node_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM node")
        .fetch_one(pool)
        .await
        .expect("count the nodes")
}

/// How many nodes of `node_type` there are.
async fn node_count_of(pool: &PgPool, node_type: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM node WHERE type = $1")
        .bind(node_type)
        .fetch_one(pool)
        .await
        .expect("count the nodes of a type")
}

/// Asserts `result` was refused by the named constraint.
fn assert_refused_by<T: Debug>(result: Result<T, sqlx::Error>, constraint: &str) {
    let error = result.expect_err("the database refuses");
    let refusing = error
        .as_database_error()
        .and_then(|database_error| database_error.constraint());
    assert_eq!(
        refusing,
        Some(constraint),
        "refused by {constraint}: {error:?}"
    );
}

/// Asserts `result` was refused with a message holding `words`.
fn assert_refused_saying<T: Debug>(result: Result<T, sqlx::Error>, words: &str) {
    let error = result.expect_err("the database refuses");
    let message = error
        .as_database_error()
        .map(|database_error| database_error.message().to_owned())
        .unwrap_or_default();
    assert!(
        message.contains(words),
        "refused saying {words:?}: {error:?}"
    );
}

// ─── A node fits its parent ────────────────────────────────────────────────

#[tokio::test]
async fn an_entry_at_top_level_is_refused() {
    let (pool, _container) = migrated().await;
    let entry = NewNode {
        parent: None,
        ..NewNode::entry(Uuid::now_v7(), "brief")
    };

    let inserted = entry.insert(&pool).await;

    assert_refused_by(inserted, "node_top_level_is_a_commission");
}

#[tokio::test]
async fn a_slot_at_top_level_is_refused() {
    let (pool, _container) = migrated().await;
    let slot = NewNode {
        parent: None,
        ..NewNode::slot(Uuid::now_v7(), &Uuid::now_v7().to_string())
    };

    let inserted = slot.insert(&pool).await;

    assert_refused_by(inserted, "node_top_level_is_a_commission");
}

#[tokio::test]
async fn an_entry_under_a_non_commission_is_refused() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await.nodes;
    let entry = NewNode::entry(commission.slots, "brief");

    let inserted = entry.insert(&pool).await;

    assert_refused_saying(inserted, "an entry sits only under a commission node");
}

#[tokio::test]
async fn a_slot_under_a_non_slots_folder_is_refused() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await.nodes;
    let slot = NewNode::slot(commission.brief, &Uuid::now_v7().to_string());

    let inserted = slot.insert(&pool).await;

    assert_refused_saying(inserted, "a slot sits only under a commission.slots node");
}

// ─── A fixed entry leaves only with its commission ─────────────────────────

#[tokio::test]
async fn deleting_a_fixed_entry_is_refused_while_its_commission_lives() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await.nodes;

    let deleted = sqlx::query("DELETE FROM node WHERE id = $1")
        .bind(commission.brief)
        .execute(&pool)
        .await;

    assert_refused_saying(deleted, "it leaves only with its commission");
}

// A bare commission node, its entries and a slot node deleted in the unit that
// wrote them: the delete takes the subtree, and the unit commits, because a
// commission node deleted since is not checked for its commission.
#[tokio::test]
async fn deleting_a_commission_node_takes_its_entries() {
    let (pool, _container) = migrated().await;
    let mut tx = pool.begin().await.expect("begin");
    let commission = bare_commission_node(&mut tx, &Uuid::now_v7().to_string()).await;
    let slot = NewNode::slot(commission.slots, &Uuid::now_v7().to_string());
    slot.insert(&mut *tx).await.expect("insert a slot node");

    let deleted = sqlx::query("DELETE FROM node WHERE id = $1")
        .bind(commission.node)
        .execute(&mut *tx)
        .await;
    let committed = tx.commit().await;

    assert!(deleted.is_ok(), "the commission node deletes: {deleted:?}");
    assert!(committed.is_ok(), "the unit commits: {committed:?}");
    assert_eq!(
        node_count(&pool).await,
        0,
        "its entries and slot went with it"
    );
}

// ─── No move, no retype ────────────────────────────────────────────────────

#[tokio::test]
async fn moving_a_node_is_refused() {
    let (pool, _container) = migrated().await;
    let first = bound_commission(&pool).await;
    let second = bound_commission(&pool).await.nodes;
    let slot = bound_slot(&pool, &first).await;

    let moved = sqlx::query("UPDATE node SET parent = $2 WHERE id = $1")
        .bind(slot.node)
        .bind(second.slots)
        .execute(&pool)
        .await;

    assert_refused_saying(moved, "keeps its place");
}

#[tokio::test]
async fn changing_a_segment_is_refused() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await;
    let slot = bound_slot(&pool, &commission).await;

    let resegmented = sqlx::query("UPDATE node SET segment = $2 WHERE id = $1")
        .bind(slot.node)
        .bind(Uuid::now_v7().to_string())
        .execute(&pool)
        .await;

    assert_refused_saying(resegmented, "keeps its place");
}

#[tokio::test]
async fn retyping_a_node_is_refused() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await.nodes;

    let retyped = sqlx::query("UPDATE node SET type = 'commission.products' WHERE id = $1")
        .bind(commission.brief)
        .execute(&pool)
        .await;

    assert_refused_saying(retyped, "keeps its place");
}

#[tokio::test]
async fn renaming_a_node_is_allowed() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await.nodes;

    let renamed =
        sqlx::query("UPDATE node SET name = 'Another name', level = 'listed' WHERE id = $1")
            .bind(commission.node)
            .execute(&pool)
            .await;

    assert!(renamed.is_ok(), "a rename passes: {renamed:?}");
}

// ─── node_id is set once ───────────────────────────────────────────────────

#[tokio::test]
async fn unpointing_a_commission_is_refused() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await;

    let unpointed = sqlx::query("UPDATE commission SET node_id = NULL WHERE id = $1")
        .bind(commission.id)
        .execute(&pool)
        .await;

    assert_refused_saying(unpointed, "node_id is set once");
}

#[tokio::test]
async fn re_pointing_a_slot_is_refused() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await;
    let slot = bound_slot(&pool, &commission).await;
    let mut tx = pool.begin().await.expect("begin");
    let other = NewNode::slot(commission.nodes.slots, &Uuid::now_v7().to_string());
    other
        .insert(&mut *tx)
        .await
        .expect("insert another slot node");

    let re_pointed = sqlx::query("UPDATE commission_slot SET node_id = $2 WHERE element_id = $1")
        .bind(slot.element_id)
        .bind(other.id)
        .execute(&mut *tx)
        .await;

    assert_refused_saying(re_pointed, "node_id is set once");
}

#[tokio::test]
async fn setting_a_node_id_from_null_is_allowed() {
    let (pool, _container) = migrated_through_den_1().await;
    let commission = seed_commission(&pool, "A commission", "private").await;

    let bound = give_it_its_nodes(&pool, commission).await;

    let pointed: Option<Uuid> = sqlx::query_scalar("SELECT node_id FROM commission WHERE id = $1")
        .bind(commission)
        .fetch_one(&pool)
        .await
        .expect("read node_id");
    assert_eq!(pointed, Some(bound.nodes.node));
}

// ─── Key changes re-check the node ─────────────────────────────────────────

#[tokio::test]
async fn changing_a_commission_s_id_re_checks_its_node_at_commit() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await;
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query("UPDATE commission SET id = $2 WHERE id = $1")
        .bind(commission.id)
        .bind(Uuid::now_v7())
        .execute(&mut *tx)
        .await
        .expect("the new id is written; the check waits for commit");

    let committed = tx.commit().await;

    assert_refused_saying(committed, "a commission points only at its own node");
}

#[tokio::test]
async fn changing_a_slot_s_element_id_re_checks_its_node_at_commit() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await;
    let slot = bound_slot(&pool, &commission).await;
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query(
        "WITH element AS (UPDATE commission_element SET id = $2 WHERE id = $1)
         UPDATE commission_slot SET element_id = $2 WHERE element_id = $1",
    )
    .bind(slot.element_id)
    .bind(Uuid::now_v7())
    .execute(&mut *tx)
    .await
    .expect("the new element id is written; the check waits for commit");

    let committed = tx.commit().await;

    assert_refused_saying(committed, "a slot points only at its own node");
}

#[tokio::test]
async fn moving_a_slot_to_another_commission_re_checks_its_node_at_commit() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await;
    bound_slot(&pool, &commission).await;
    let other = bound_commission(&pool).await;
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query(
        "WITH tab AS (UPDATE commission_tab SET commission_id = $2 WHERE commission_id = $1),
              element AS (UPDATE commission_element SET commission_id = $2 WHERE commission_id = $1)
         UPDATE commission_slot SET commission_id = $2 WHERE commission_id = $1",
    )
    .bind(commission.id)
    .bind(other.id)
    .execute(&mut *tx)
    .await
    .expect("the slot moves; the check waits for commit");

    let committed = tx.commit().await;

    assert_refused_saying(committed, "a slot points only at its own node");
}

// ─── The stored vocabularies ───────────────────────────────────────────────

#[tokio::test]
async fn a_level_outside_the_three_is_refused() {
    let (pool, _container) = migrated().await;

    for level in ["secret", "Private", ""] {
        let node = NewNode {
            level: level.to_owned(),
            ..NewNode::commission(&Uuid::now_v7().to_string())
        };
        let inserted = node.insert(&pool).await;
        assert_refused_by(inserted, "node_level_known");
    }
}

#[tokio::test]
async fn a_type_outside_the_eight_words_is_refused() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await.nodes;

    for node_type in ["folder", "commission.extras", "Slot"] {
        let node = NewNode {
            node_type: node_type.to_owned(),
            ..NewNode::slot(commission.node, "extras")
        };
        let inserted = node.insert(&pool).await;
        assert_refused_by(inserted, "node_type_known");
    }
}

// ─── The segment rule ──────────────────────────────────────────────────────

#[tokio::test]
async fn a_segment_of_the_allowed_characters_is_accepted() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await.nodes;
    let longest = "a".repeat(2048);
    let accepted = [
        "under_score",
        "per%25cent",
        "tilde~",
        "colon:dash-dot.",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._:%~-",
        "...",
        longest.as_str(),
    ];

    // A slot node commits only with its Slot, so each insert is checked on a unit dropped after.
    let mut tx = pool.begin().await.expect("begin");
    for segment in accepted {
        let node = NewNode::slot(commission.slots, segment);
        let inserted = node.insert(&mut *tx).await;
        assert!(inserted.is_ok(), "{segment:?} is a segment: {inserted:?}");
    }
    tx.rollback().await.expect("the unit ends");
}

#[tokio::test]
async fn a_segment_outside_the_rule_is_refused() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await.nodes;
    let too_long = "a".repeat(2049);
    let refused = [
        "é",
        "a/b",
        "a b",
        "a\nb",
        "a\\b",
        ".",
        "..",
        "",
        too_long.as_str(),
    ];

    for segment in refused {
        let node = NewNode::slot(commission.slots, segment);
        let inserted = node.insert(&pool).await;
        assert_refused_by(inserted, "node_segment_shape");
    }
}

#[tokio::test]
async fn a_second_top_level_row_with_the_same_segment_is_refused() {
    let (pool, _container) = migrated().await;
    let segment = Uuid::now_v7().to_string();
    let mut tx = pool.begin().await.expect("begin");
    NewNode::commission(&segment)
        .insert(&mut *tx)
        .await
        .expect("the first top-level node");

    let second = NewNode::commission(&segment).insert(&mut *tx).await;

    assert_refused_by(second, "node_parent_segment_key");
}

// ─── A node isn't deleted from under its thing ─────────────────────────────

#[tokio::test]
async fn deleting_a_node_its_commission_points_at_is_refused_at_commit() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await;
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query("DELETE FROM node WHERE id = $1")
        .bind(commission.nodes.node)
        .execute(&mut *tx)
        .await
        .expect("the delete waits for commit");

    let committed = tx.commit().await;

    assert_refused_by(committed, "commission_node_id_fkey");
}

#[tokio::test]
async fn deleting_a_node_its_slot_points_at_is_refused_at_commit() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await;
    let slot = bound_slot(&pool, &commission).await;
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query("DELETE FROM node WHERE id = $1")
        .bind(slot.node)
        .execute(&mut *tx)
        .await
        .expect("the delete waits for commit");

    let committed = tx.commit().await;

    assert_refused_by(committed, "commission_slot_node_id_fkey");
}

#[tokio::test]
async fn truncating_the_commission_tables_leaves_no_node() {
    let (pool, _container) = migrated().await;
    let first = bound_commission(&pool).await;
    let second = bound_commission(&pool).await;
    bound_slot(&pool, &first).await;
    bound_slot(&pool, &second).await;
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query("TRUNCATE commission CASCADE")
        .execute(&mut *tx)
        .await
        .expect("the truncate runs");

    let committed = tx.commit().await;

    assert!(committed.is_ok(), "the truncate commits: {committed:?}");
    assert_eq!(node_count(&pool).await, 0, "no node outlives the truncate");
}

#[tokio::test]
async fn truncating_commission_slot_leaves_no_slot_node() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await;
    bound_slot(&pool, &commission).await;
    bound_slot(&pool, &commission).await;
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query("TRUNCATE commission_slot")
        .execute(&mut *tx)
        .await
        .expect("the truncate runs");

    let committed = tx.commit().await;

    assert!(committed.is_ok(), "the truncate commits: {committed:?}");
    assert_eq!(
        node_count_of(&pool, "slot").await,
        0,
        "no slot node remains"
    );
    assert_eq!(
        node_count(&pool).await,
        7,
        "the commission keeps its node and entries"
    );
}

// ─── A commission node has its commission ──────────────────────────────────

#[tokio::test]
async fn a_commission_node_without_its_commission_is_refused_at_commit() {
    let (pool, _container) = migrated().await;
    let mut tx = pool.begin().await.expect("begin");
    let bare = bare_commission_node(&mut tx, &Uuid::now_v7().to_string()).await;

    let committed = tx.commit().await;

    assert_refused_saying(
        committed,
        "a commission node stands only with its commission",
    );
    let kept: i64 = sqlx::query_scalar("SELECT count(*) FROM node WHERE id = $1")
        .bind(bare.node)
        .fetch_one(&pool)
        .await
        .expect("count the node");
    assert_eq!(kept, 0, "the refused unit keeps no node");
}

// ─── A thing points only at its own node ───────────────────────────────────

#[tokio::test]
async fn a_commission_pointing_at_another_commission_s_node_is_refused_at_commit() {
    let (pool, _container) = migrated_through_den_1().await;
    let pointing = seed_commission(&pool, "A pointing commission", "private").await;
    let other = seed_commission(&pool, "Another commission", "private").await;
    let mut tx = pool.begin().await.expect("begin");
    let others_node = bare_commission_node(&mut tx, &other.to_string()).await;
    sqlx::query("UPDATE commission SET node_id = $2 WHERE id = $1")
        .bind(pointing)
        .bind(others_node.node)
        .execute(&mut *tx)
        .await
        .expect("the pointer waits for commit");

    let committed = tx.commit().await;

    assert_refused_saying(committed, "a commission points only at its own node");
}

#[tokio::test]
async fn a_commission_pointing_at_a_non_commission_node_is_refused_at_commit() {
    let (pool, _container) = migrated_through_den_1().await;
    let pointing = seed_commission(&pool, "A pointing commission", "private").await;
    let mut tx = pool.begin().await.expect("begin");
    let its_nodes = bare_commission_node(&mut tx, &pointing.to_string()).await;
    sqlx::query("UPDATE commission SET node_id = $2 WHERE id = $1")
        .bind(pointing)
        .bind(its_nodes.brief)
        .execute(&mut *tx)
        .await
        .expect("the pointer waits for commit");

    let committed = tx.commit().await;

    assert_refused_saying(committed, "a commission points only at its own node");
}

#[tokio::test]
async fn a_slot_pointing_at_a_slot_node_under_another_commission_s_slots_is_refused_at_commit() {
    let (pool, _container) = migrated().await;
    let own = bound_commission(&pool).await;
    let other = bound_commission(&pool).await;
    let mut tx = pool.begin().await.expect("begin");
    let slot = slot_element(&mut tx, own.id).await;
    let elsewhere = NewNode::slot(other.nodes.slots, &slot.to_string());
    elsewhere
        .insert(&mut *tx)
        .await
        .expect("insert a slot node under the other commission");
    slot_row_at(&mut tx, slot, own.id, elsewhere.id)
        .await
        .expect("the pointer waits for commit");

    let committed = tx.commit().await;

    assert_refused_saying(committed, "a slot points only at its own node");
}

#[tokio::test]
async fn a_slot_pointing_at_a_node_with_another_segment_is_refused_at_commit() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await;
    let mut tx = pool.begin().await.expect("begin");
    let slot = slot_element(&mut tx, commission.id).await;
    let misnamed = NewNode::slot(commission.nodes.slots, &Uuid::now_v7().to_string());
    misnamed
        .insert(&mut *tx)
        .await
        .expect("insert a slot node with another segment");
    slot_row_at(&mut tx, slot, commission.id, misnamed.id)
        .await
        .expect("the pointer waits for commit");

    let committed = tx.commit().await;

    assert_refused_saying(committed, "a slot points only at its own node");
}

/// node_fits_its_parent keeps a slot node from sitting under anything but a
/// `commission.slots` node, so this test switches it off inside its own unit
/// to reach that state and check that the commit-time check refuses it too.
/// The refused commit rolls the switch back with everything else.
#[tokio::test]
async fn a_slot_whose_node_sits_under_a_non_slots_parent_is_refused_at_commit() {
    let (pool, _container) = migrated().await;
    let commission = bound_commission(&pool).await;
    let mut tx = pool.begin().await.expect("begin");
    sqlx::query("ALTER TABLE node DISABLE TRIGGER node_fits_its_parent")
        .execute(&mut *tx)
        .await
        .expect("switch off the insert-time fit");
    let slot = slot_element(&mut tx, commission.id).await;
    let under_brief = NewNode::slot(commission.nodes.brief, &slot.to_string());
    under_brief
        .insert(&mut *tx)
        .await
        .expect("insert a slot node under the brief");
    slot_row_at(&mut tx, slot, commission.id, under_brief.id)
        .await
        .expect("the pointer waits for commit");

    let committed = tx.commit().await;

    assert_refused_saying(committed, "a slot points only at its own node");
}

#[tokio::test]
async fn a_commission_created_and_deleted_in_one_unit_commits() {
    let (pool, _container) = migrated().await;
    let id = Uuid::now_v7();
    let mut tx = pool.begin().await.expect("begin");
    let nodes = bare_commission_node(&mut tx, &id.to_string()).await;
    sqlx::query(
        "INSERT INTO commission (id, owner_id, lifecycle, created_at, node_id)
         VALUES ($1, $2, 'draft', now(), $3)",
    )
    .bind(id)
    .bind(crate::node_backfill::OWNER_DID)
    .bind(nodes.node)
    .execute(&mut *tx)
    .await
    .expect("create the commission");
    sqlx::query("DELETE FROM commission WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .expect("delete it again");

    let committed = tx.commit().await;

    assert!(committed.is_ok(), "the unit commits: {committed:?}");
    assert_eq!(node_count(&pool).await, 0, "its nodes went with it");
}

// ─── The shared segment cases ──────────────────────────────────────────────

/// The case file the store's check, Atta's rule and kernel's all answer to, from this
/// crate's manifest directory.
const SHARED_SEGMENT_CASES: &str = "../../../contract/golden/segments.json";

/// One shared case: what it shows, and its segment text.
struct SegmentCase {
    description: String,
    segment: String,
}

/// A shared case read from its JSON object; a `repeat` case is its text `times` times over.
fn segment_case(value: &serde_json::Value) -> SegmentCase {
    let description = value["description"].as_str().expect("a case's description");
    let segment = match value["segment"].as_str() {
        Some(segment) => segment.to_owned(),
        None => {
            let text = value["repeat"].as_str().expect("a segment or a repeat");
            let times = value["times"].as_u64().expect("a repeat's times");
            let times = usize::try_from(times).expect("the times fit a usize");
            text.repeat(times)
        }
    };
    SegmentCase {
        description: description.to_owned(),
        segment,
    }
}

/// The accepted and the refused cases of the shared case file.
fn segment_cases() -> (Vec<SegmentCase>, Vec<SegmentCase>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SHARED_SEGMENT_CASES);
    let text = std::fs::read_to_string(&path).expect("reads the shared case file");
    let document: serde_json::Value = serde_json::from_str(&text).expect("the case file is JSON");
    let read = |list: &str| -> Vec<SegmentCase> {
        document[list]
            .as_array()
            .expect("a list of cases")
            .iter()
            .map(segment_case)
            .collect()
    };
    (read("accepted"), read("refused"))
}

#[tokio::test]
async fn segments_follow_the_shared_cases() {
    let (pool, _container) = migrated().await;
    let (accepted, refused) = segment_cases();
    let holds =
        |cases: &[SegmentCase], segment: &str| cases.iter().any(|case| case.segment == segment);
    assert!(holds(&accepted, "a_b"), "the underscore case is present");
    assert!(
        holds(&accepted, &"a".repeat(2048)),
        "the 2048-byte case is present"
    );
    assert!(holds(&refused, "\u{e9}"), "the accent case is present");
    assert!(holds(&refused, ".."), "the dot-dot case is present");
    assert!(
        holds(&refused, &"a".repeat(2049)),
        "the 2049-byte case is present"
    );

    // An accepted commission node is inserted in a unit rolled back after: committed bare,
    // it would stand without its commission.
    for case in &accepted {
        let mut tx = pool.begin().await.expect("begin");
        let inserted = NewNode::commission(&case.segment).insert(&mut *tx).await;
        assert!(inserted.is_ok(), "{}: {inserted:?}", case.description);
        tx.rollback().await.expect("roll the accepted node back");
    }
    for case in &refused {
        let inserted = NewNode::commission(&case.segment).insert(&pool).await;
        let refusing = inserted
            .as_ref()
            .err()
            .and_then(|error| error.as_database_error())
            .and_then(|database_error| database_error.constraint());
        assert_eq!(refusing, Some("node_segment_shape"), "{}", case.description);
    }
}

// ─── The closed type vocabulary ────────────────────────────────────────────

#[tokio::test]
async fn the_type_check_lists_exactly_node_type_s_words() {
    use strum::VariantArray;

    let (pool, _container) = migrated().await;
    let definition: String = sqlx::query_scalar(
        "SELECT pg_get_constraintdef(oid) FROM pg_constraint WHERE conname = 'node_type_known'",
    )
    .fetch_one(&pool)
    .await
    .expect("the node_type_known check");

    let mut checked: Vec<&str> = definition.split('\'').skip(1).step_by(2).collect();
    checked.sort_unstable();
    let mut kernel_words: Vec<&'static str> = domain::elements::node::NodeType::VARIANTS
        .iter()
        .map(|node_type| <&'static str>::from(*node_type))
        .collect();
    kernel_words.sort_unstable();
    assert_eq!(checked, kernel_words, "{definition}");
}

// ─── The node port: the shared conformance suite ───────────────────────────

/// The node port over the PostgreSQL adapter, on one fresh database per case.
mod contract {
    use std::sync::Arc;

    use adapter_pg::{PgDatabase, PgNodeStore};
    use domain::ports::{Database, NodeStore};
    use test_support::seeder::{CommissionSeeder, SlotSeeder};

    use crate::seed::{MintingSeeder, MintingSlotSeeder};

    /// A fresh migrated database's unit factory, node store, commission seeder and Slot
    /// seeder, and the database kept alive.
    async fn fresh() -> (
        Arc<dyn Database>,
        Arc<dyn NodeStore>,
        Arc<dyn CommissionSeeder>,
        Arc<dyn SlotSeeder>,
        impl Sized,
    ) {
        let (pool, db) = test_support::pg::fresh_pool().await;
        let database: Arc<dyn Database> = Arc::new(PgDatabase::new(pool.clone()));
        let store: Arc<dyn NodeStore> = Arc::new(PgNodeStore::new(pool.clone()));
        let seeder: Arc<dyn CommissionSeeder> = Arc::new(MintingSeeder);
        let slots: Arc<dyn SlotSeeder> = Arc::new(MintingSlotSeeder(pool));
        (database, store, seeder, slots, db)
    }

    test_support::node_contract_tests!(fresh);
}
