//! The commission content tree over PostgreSQL (ZMVP-71/72/73), against a
//! throwaway container: every commission is born with (or backfilled to) a root
//! surface, the owner grows surfaces under any existing surface in append
//! order, components grow as leaves whose opaque payload round-trips (and under
//! which nothing grows), the whole tree loads back assembled, and pruning takes
//! a node's whole subtree (root refused, siblings renumbered). Requires a
//! container runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            Commission, CommissionId, CommissionTitle, NewComponent, NewSurface, NodeId, NodeKind,
            SurfaceMode,
        },
        did::Did,
        user::User,
    },
    ports::{
        CannotRemoveRoot, CommissionStore, Database, NodeNotFound, ParentNodeNotFound,
        ParentNotASurface,
    },
};
use serde_json::json;

/// The ZMVP-71 migration (create `commission_node` + root backfill), as sqlx
/// numbers it. The backfill test runs everything *before* this version, seeds
/// pre-tree commissions, then lets the full migrator catch up.
const COMMISSION_NODE_MIGRATION: i64 = 20260705150000;

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// A fresh, empty private database with NO migrations applied (the backfill
/// tests drive the migrator themselves).
async fn bare_pool() -> (PgPool, impl Sized) {
    let db = test_support::pg::bare_db().await;
    let pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    (pool, db)
}

/// Recognize a visitor in its own committed unit of work
/// (`commission_node.created_by` references `users(id)`).
async fn provision(pool: &PgPool, did: &str) -> User {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// Create a commission (which mints its root) in one committed unit of work.
async fn create_commission(pool: &PgPool, owner: &User, title: &str) -> Commission {
    let commission = Commission::create(
        CommissionTitle::try_new(title).expect("valid title"),
        owner.id,
        Utc::now(),
        None,
    );
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .create(&commission)
        .await
        .expect("create commission");
    uow.commit().await.expect("commit");
    commission
}

// AC1 (pg) — creating a commission mints its root surface in the same unit of
// work: the loaded tree is exactly one Total root (birth = Private) carrying the
// owner and the commission's creation instant, with no children.
#[tokio::test]
async fn creating_a_commission_mints_its_root_surface() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:tree-owner").await;
    let commission = create_commission(&pool, &owner, "Rooted").await;

    let store = PgCommissionStore::new(pool.clone());
    let tree = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("a created commission always has a tree");
    assert!(matches!(
        tree.root.kind,
        NodeKind::Surface {
            mode: SurfaceMode::Total
        }
    ));
    assert_eq!(tree.root.created_by, owner.id);
    // Compare two DB round-trips (timestamptz is microsecond-precise, so the
    // in-memory nanosecond value wouldn't match).
    let stored = store
        .find(commission.id)
        .await
        .expect("find")
        .expect("commission exists");
    assert_eq!(
        tree.root.created_at, stored.created_at,
        "the root is born with the commission"
    );
    assert!(tree.root.children.is_empty());
}

// AC2/AC3 (pg) — surfaces grow under any existing surface (root and non-root),
// siblings keep append order, and every new surface is born Total.
#[tokio::test]
async fn add_surface_grows_the_tree_in_append_order() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:gardener").await;
    let commission = create_commission(&pool, &owner, "Growing").await;

    let store = PgCommissionStore::new(pool.clone());
    let root = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists")
        .root
        .id;

    let first = NewSurface::under(commission.id, root, owner.id, Utc::now());
    let second = NewSurface::under(commission.id, root, owner.id, Utc::now());
    let nested = NewSurface::under(commission.id, first.id, owner.id, Utc::now());
    let (first_id, second_id, nested_id) = (first.id, second.id, nested.id);

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        commissions.add_surface(&first).await.expect("first");
        commissions.add_surface(&second).await.expect("second");
        commissions.add_surface(&nested).await.expect("nested");
    }
    uow.commit().await.expect("commit");

    let tree = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 2);
    assert_eq!(tree.root.children[0].id, first_id, "append order holds");
    assert_eq!(tree.root.children[1].id, second_id);
    assert_eq!(
        tree.root.children[0].children[0].id, nested_id,
        "a surface grows under any existing surface"
    );
    assert!(tree.root.children.iter().all(|child| matches!(
        child.kind,
        NodeKind::Surface {
            mode: SurfaceMode::Total
        }
    )));
}

// ZMVP-71 (pg) — an absent parent id and a parent in another commission's tree
// both refuse with ParentNodeNotFound (indistinguishably), and a refused unit
// leaves no row behind.
#[tokio::test]
async fn add_surface_refuses_absent_and_foreign_parents() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:prober").await;
    let mine = create_commission(&pool, &owner, "Mine").await;
    let theirs = {
        let other = provision(&pool, "did:plc:someone-else").await;
        create_commission(&pool, &other, "Theirs").await
    };

    let store = PgCommissionStore::new(pool.clone());
    let their_root = store
        .load_tree(theirs.id)
        .await
        .expect("load")
        .expect("their tree exists")
        .root
        .id;

    let db = PgDatabase::new(pool.clone());

    let fabricated = NewSurface::under(
        mine.id,
        NodeId::new(uuid::Uuid::now_v7()),
        owner.id,
        Utc::now(),
    );
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .add_surface(&fabricated)
        .await
        .expect_err("absent parent refuses");
    assert!(
        err.downcast_ref::<ParentNodeNotFound>().is_some(),
        "absent parent surfaces as ParentNodeNotFound, got: {err:?}"
    );
    uow.rollback().await.expect("rollback");

    let cross = NewSurface::under(mine.id, their_root, owner.id, Utc::now());
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .add_surface(&cross)
        .await
        .expect_err("foreign parent refuses");
    assert!(
        err.downcast_ref::<ParentNodeNotFound>().is_some(),
        "a foreign-tree parent is indistinguishable from an absent one, got: {err:?}"
    );
    uow.rollback().await.expect("rollback");

    let tree = store
        .load_tree(mine.id)
        .await
        .expect("load")
        .expect("tree exists");
    assert!(tree.root.children.is_empty(), "no refused write landed");
}

// load_tree for an id nobody created is None, mirroring `find`.
#[tokio::test]
async fn load_tree_answers_none_for_an_unknown_commission() {
    let (pool, _container) = fresh_pool().await;
    let store = PgCommissionStore::new(pool);
    assert!(
        store
            .load_tree(CommissionId::new(uuid::Uuid::now_v7()))
            .await
            .expect("load")
            .is_none()
    );
}

// ZMVP-72 AC1/AC2/AC3 (pg) — a component grows as a leaf under a surface (root
// and non-root alike), stores a NULL mode (the surface-XOR-mode CHECK holds),
// and its opaque jsonb payload reads back exactly as written — nested
// structure, unicode, numbers, booleans, in-payload nulls, even a top-level
// JSON null (jsonb 'null', not SQL NULL).
#[tokio::test]
async fn add_component_grows_a_leaf_whose_payload_round_trips() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:componenter").await;
    let commission = create_commission(&pool, &owner, "Componented").await;

    let store = PgCommissionStore::new(pool.clone());
    let root = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists")
        .root
        .id;

    let surface = NewSurface::under(commission.id, root, owner.id, Utc::now());
    let payload = json!({
        "kind": "text",
        "body": "Reference: 三毛猫 🐾 — \"line\\break\"",
        "revision": 3,
        "ratio": 1.5,
        "flags": [true, false, null],
        "nested": { "empty": {}, "list": [] },
    });
    let on_root = NewComponent::under(commission.id, root, payload.clone(), owner.id, Utc::now());
    let nested = NewComponent::under(commission.id, surface.id, json!(null), owner.id, Utc::now());
    let (surface_id, on_root_id, nested_id) = (surface.id, on_root.id, nested.id);

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        commissions.add_surface(&surface).await.expect("surface");
        commissions.add_component(&on_root).await.expect("on root");
        commissions.add_component(&nested).await.expect("nested");
    }
    uow.commit().await.expect("commit");

    let tree = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 2);
    assert_eq!(tree.root.children[0].id, surface_id, "append order holds");
    let component = &tree.root.children[1];
    assert_eq!(component.id, on_root_id);
    assert!(
        matches!(component.kind, NodeKind::Component),
        "a component carries no mode of its own, got {:?}",
        component.kind
    );
    assert_eq!(component.created_by, owner.id);
    assert_eq!(component.payload, payload, "the payload is opaque (AC3)");
    assert!(component.children.is_empty());

    let nested = &tree.root.children[0].children[0];
    assert_eq!(nested.id, nested_id, "grows under a non-root surface too");
    assert_eq!(
        nested.payload,
        json!(null),
        "even a top-level JSON null round-trips verbatim"
    );
}

// ZMVP-72 AC1/AC2 (pg) — components are leaves: growing ANYTHING under one —
// another component or a surface — refuses with ParentNotASurface, and the
// refused unit leaves no row behind.
#[tokio::test]
async fn nothing_grows_under_a_component() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:leaf-prober").await;
    let commission = create_commission(&pool, &owner, "Leafy").await;

    let store = PgCommissionStore::new(pool.clone());
    let root = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists")
        .root
        .id;

    let component = NewComponent::under(commission.id, root, json!({}), owner.id, Utc::now());
    let component_id = component.id;
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .add_component(&component)
        .await
        .expect("component lands");
    uow.commit().await.expect("commit");

    let child_component =
        NewComponent::under(commission.id, component_id, json!({}), owner.id, Utc::now());
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .add_component(&child_component)
        .await
        .expect_err("a component under a component refuses");
    assert!(
        err.downcast_ref::<ParentNotASurface>().is_some(),
        "expected ParentNotASurface, got: {err:?}"
    );
    uow.rollback().await.expect("rollback");

    let child_surface = NewSurface::under(commission.id, component_id, owner.id, Utc::now());
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .add_surface(&child_surface)
        .await
        .expect_err("a surface under a component refuses too");
    assert!(
        err.downcast_ref::<ParentNotASurface>().is_some(),
        "expected ParentNotASurface, got: {err:?}"
    );
    uow.rollback().await.expect("rollback");

    let tree = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 1);
    assert!(
        tree.root.children[0].children.is_empty(),
        "a component never has children"
    );
}

// ZMVP-72 (pg) — the component's parent must exist in THIS commission's tree:
// a fabricated parent id and a surface belonging to another commission both
// refuse with ParentNodeNotFound (indistinguishably — never ParentNotASurface,
// which would leak what a foreign node is), and no row lands.
#[tokio::test]
async fn add_component_refuses_absent_and_foreign_parents() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:component-prober").await;
    let mine = create_commission(&pool, &owner, "Mine").await;
    let theirs = {
        let other = provision(&pool, "did:plc:component-victim").await;
        create_commission(&pool, &other, "Theirs").await
    };

    let store = PgCommissionStore::new(pool.clone());
    let their_root = store
        .load_tree(theirs.id)
        .await
        .expect("load")
        .expect("their tree exists")
        .root
        .id;

    let db = PgDatabase::new(pool.clone());

    let fabricated = NewComponent::under(
        mine.id,
        NodeId::new(uuid::Uuid::now_v7()),
        json!({}),
        owner.id,
        Utc::now(),
    );
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .add_component(&fabricated)
        .await
        .expect_err("absent parent refuses");
    assert!(
        err.downcast_ref::<ParentNodeNotFound>().is_some(),
        "absent parent surfaces as ParentNodeNotFound, got: {err:?}"
    );
    uow.rollback().await.expect("rollback");

    let cross = NewComponent::under(mine.id, their_root, json!({}), owner.id, Utc::now());
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .add_component(&cross)
        .await
        .expect_err("foreign parent refuses");
    assert!(
        err.downcast_ref::<ParentNodeNotFound>().is_some(),
        "a foreign-tree parent is indistinguishable from an absent one, got: {err:?}"
    );
    uow.rollback().await.expect("rollback");

    let tree = store
        .load_tree(mine.id)
        .await
        .expect("load")
        .expect("tree exists");
    assert!(tree.root.children.is_empty(), "no refused write landed");
}

/// The `position` values of `parent`'s current children, ascending — read
/// straight off the table, so a test can assert the renumbering invariant
/// (contiguous from 0) that the assembled tree deliberately hides.
async fn sibling_positions(pool: &PgPool, parent: NodeId) -> Vec<(uuid::Uuid, i32)> {
    sqlx::query_as::<_, (uuid::Uuid, i32)>(
        "SELECT id, position FROM commission_node WHERE parent = $1 ORDER BY position",
    )
    .bind(*parent)
    .fetch_all(pool)
    .await
    .expect("read sibling positions")
}

/// Runs `remove_node` in its own committed unit of work.
async fn remove_node(pool: &PgPool, commission: CommissionId, node: NodeId) -> anyhow::Result<()> {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await?;
    uow.commissions().remove_node(commission, node).await?;
    uow.commit().await
}

// ZMVP-73 AC1 (pg) — removing a mid-tree surface takes its ENTIRE subtree (a
// component and a nested surface with its own component ride the
// self-referential cascade), leaves the other siblings intact, and renumbers
// the remaining sibling group to contiguous positions in the same transaction.
#[tokio::test]
async fn remove_surface_takes_its_whole_subtree_and_renumbers() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:pruner").await;
    let commission = create_commission(&pool, &owner, "Pruned").await;

    let store = PgCommissionStore::new(pool.clone());
    let root = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists")
        .root
        .id;

    // root -> [first, doomed, last]; under doomed: a component and a surface
    // holding another component.
    let first = NewSurface::under(commission.id, root, owner.id, Utc::now());
    let doomed = NewSurface::under(commission.id, root, owner.id, Utc::now());
    let last = NewSurface::under(commission.id, root, owner.id, Utc::now());
    let in_doomed = NewComponent::under(
        commission.id,
        doomed.id,
        json!({"kind": "text"}),
        owner.id,
        Utc::now(),
    );
    let nested = NewSurface::under(commission.id, doomed.id, owner.id, Utc::now());
    let in_nested = NewComponent::under(commission.id, nested.id, json!({}), owner.id, Utc::now());
    let (first_id, doomed_id, last_id) = (first.id, doomed.id, last.id);

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        commissions.add_surface(&first).await.expect("first");
        commissions.add_surface(&doomed).await.expect("doomed");
        commissions.add_surface(&last).await.expect("last");
        commissions.add_component(&in_doomed).await.expect("leaf");
        commissions.add_surface(&nested).await.expect("nested");
        commissions
            .add_component(&in_nested)
            .await
            .expect("nested leaf");
    }
    uow.commit().await.expect("commit");

    remove_node(&pool, commission.id, doomed_id)
        .await
        .expect("removal succeeds");

    let tree = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists");
    assert_eq!(
        tree.root.children.len(),
        2,
        "the surface and its whole subtree went together"
    );
    assert_eq!(tree.root.children[0].id, first_id, "sibling order holds");
    assert_eq!(tree.root.children[1].id, last_id);
    assert!(
        tree.root.children.iter().all(|c| c.children.is_empty()),
        "nothing of the subtree survives"
    );
    assert_eq!(
        sibling_positions(&pool, root).await,
        vec![(*first_id, 0), (*last_id, 1)],
        "the remaining siblings renumber to contiguous positions"
    );
}

// ZMVP-73 AC2 (pg) — removing a component removes just that leaf; its siblings
// survive in order with contiguous positions.
#[tokio::test]
async fn remove_component_removes_only_the_leaf() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:leaf-pruner").await;
    let commission = create_commission(&pool, &owner, "Leaf-pruned").await;

    let store = PgCommissionStore::new(pool.clone());
    let root = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists")
        .root
        .id;

    let doomed = NewComponent::under(
        commission.id,
        root,
        json!({"kind": "text"}),
        owner.id,
        Utc::now(),
    );
    let surface = NewSurface::under(commission.id, root, owner.id, Utc::now());
    let kept = NewComponent::under(commission.id, root, json!({}), owner.id, Utc::now());
    let (doomed_id, surface_id, kept_id) = (doomed.id, surface.id, kept.id);

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        commissions.add_component(&doomed).await.expect("doomed");
        commissions.add_surface(&surface).await.expect("surface");
        commissions.add_component(&kept).await.expect("kept");
    }
    uow.commit().await.expect("commit");

    remove_node(&pool, commission.id, doomed_id)
        .await
        .expect("removal succeeds");

    let tree = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 2, "only the one leaf went");
    assert_eq!(tree.root.children[0].id, surface_id, "order holds");
    assert_eq!(tree.root.children[1].id, kept_id);
    assert_eq!(
        sibling_positions(&pool, root).await,
        vec![(*surface_id, 0), (*kept_id, 1)],
        "positions renumber contiguously"
    );
}

// ZMVP-73 AC3 (pg) — the root surface refuses removal with CannotRemoveRoot,
// and the whole tree is untouched.
#[tokio::test]
async fn removing_the_root_is_refused() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:root-pruner").await;
    let commission = create_commission(&pool, &owner, "Rooted still").await;

    let store = PgCommissionStore::new(pool.clone());
    let root = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists")
        .root
        .id;

    let child = NewSurface::under(commission.id, root, owner.id, Utc::now());
    let child_id = child.id;
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions().add_surface(&child).await.expect("child");
    uow.commit().await.expect("commit");

    let err = remove_node(&pool, commission.id, root)
        .await
        .expect_err("the root refuses removal");
    assert!(
        err.downcast_ref::<CannotRemoveRoot>().is_some(),
        "expected CannotRemoveRoot, got: {err:?}"
    );

    let tree = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists");
    assert_eq!(tree.root.id, root, "the root survives");
    assert_eq!(tree.root.children[0].id, child_id, "so does its subtree");
}

// ZMVP-73 (pg) — a fabricated node id and a node belonging to another
// commission both refuse with NodeNotFound (indistinguishably); someone else's
// ROOT through my commission id is also NodeNotFound — never CannotRemoveRoot,
// which would leak what a foreign node is — and nothing is removed anywhere.
#[tokio::test]
async fn remove_refuses_absent_and_foreign_nodes() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:remove-prober").await;
    let mine = create_commission(&pool, &owner, "Mine").await;
    let theirs = {
        let other = provision(&pool, "did:plc:remove-victim").await;
        create_commission(&pool, &other, "Theirs").await
    };

    let store = PgCommissionStore::new(pool.clone());
    let their_root = store
        .load_tree(theirs.id)
        .await
        .expect("load")
        .expect("their tree exists")
        .root
        .id;

    let err = remove_node(&pool, mine.id, NodeId::new(uuid::Uuid::now_v7()))
        .await
        .expect_err("a fabricated node refuses");
    assert!(
        err.downcast_ref::<NodeNotFound>().is_some(),
        "expected NodeNotFound, got: {err:?}"
    );

    let err = remove_node(&pool, mine.id, their_root)
        .await
        .expect_err("a foreign node refuses");
    assert!(
        err.downcast_ref::<NodeNotFound>().is_some(),
        "a foreign root is indistinguishable from an absent node, got: {err:?}"
    );

    assert!(
        store.load_tree(theirs.id).await.expect("load").is_some(),
        "the foreign tree is untouched"
    );
}

// AC1, the retroactive half — commissions created BEFORE the tree existed get a
// root backfilled by the migration, mode mapped from the flat visibility column
// (private -> total, listed -> presentation, public -> description), owned by
// the commission's owner and stamped with its creation instant.
#[tokio::test]
async fn the_migration_backfills_a_root_for_pre_tree_commissions() {
    let (pool, _container) = bare_pool().await;

    // Run every migration BEFORE the commission_node one.
    let mut pre_tree = adapter_pg::migrator();
    let migrations: Vec<_> = pre_tree
        .migrations
        .iter()
        .filter(|m| m.version < COMMISSION_NODE_MIGRATION)
        .cloned()
        .collect();
    assert!(
        !migrations.is_empty() && migrations.len() < pre_tree.migrations.len(),
        "the version constant matches an embedded migration"
    );
    pre_tree.migrations = migrations.into();
    pre_tree.run(&pool).await.expect("pre-tree migrations run");

    // Seed a pre-tree world: an owner and one commission per visibility value,
    // exactly as ZMVP-65 wrote them (no tree anywhere).
    let owner = provision(&pool, "did:plc:early-adopter").await;
    let mut seeded = Vec::new();
    for visibility in ["private", "listed", "public"] {
        let id = uuid::Uuid::now_v7();
        let created_at = Utc::now();
        sqlx::query(
            "INSERT INTO commission (id, title, owner_id, lifecycle, visibility, created_at)
             VALUES ($1, $2, $3, 'draft', $4, $5)",
        )
        .bind(id)
        .bind(format!("Pre-tree {visibility}"))
        .bind(*owner.id)
        .bind(visibility)
        .bind(created_at)
        .execute(&pool)
        .await
        .expect("seed pre-tree commission");
        seeded.push((CommissionId::new(id), visibility, created_at));
    }

    // The world catches up: the remaining migrations (including the backfill) run.
    adapter_pg::migrate(&pool).await.expect("catch-up migrates");

    let store = PgCommissionStore::new(pool.clone());
    for (id, visibility, _created_at) in seeded {
        let tree = store
            .load_tree(id)
            .await
            .expect("load")
            .expect("the backfill minted a root");
        let expected = match visibility {
            "private" => SurfaceMode::Total,
            "listed" => SurfaceMode::Presentation,
            "public" => SurfaceMode::Description,
            _ => unreachable!(),
        };
        assert!(
            matches!(tree.root.kind, NodeKind::Surface { mode } if mode == expected),
            "{visibility} maps to {expected:?}, got {:?}",
            tree.root.kind
        );
        assert_eq!(tree.root.created_by, owner.id);
        // Both values round-tripped through timestamptz, so they compare exactly.
        let stored = store
            .find(id)
            .await
            .expect("find")
            .expect("commission exists");
        assert_eq!(
            tree.root.created_at, stored.created_at,
            "the backfilled root carries the commission's creation instant"
        );
        assert!(tree.root.children.is_empty());
    }
}
