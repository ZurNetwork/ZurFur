//! The commission content tree over PostgreSQL (ZMVP-71), against a throwaway
//! container: every commission is born with (or backfilled to) a root surface,
//! the owner grows surfaces under any existing surface in append order, and the
//! whole tree loads back assembled. Requires a container runtime socket
//! (DOCKER_HOST honored).

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            Commission, CommissionId, CommissionTitle, NewSurface, NodeId, NodeKind, SurfaceMode,
        },
        did::Did,
        user::User,
    },
    ports::{CommissionStore, Database, ParentNodeNotFound},
};
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};

/// The ZMVP-71 migration (create `commission_node` + root backfill), as sqlx
/// numbers it. The backfill test runs everything *before* this version, seeds
/// pre-tree commissions, then lets the full migrator catch up.
const COMMISSION_NODE_MIGRATION: i64 = 20260705150000;

/// Boots a fresh database and runs all migrations. The container is returned so
/// the caller keeps it alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    let (pool, container) = bare_pool().await;
    adapter_pg::migrate(&pool).await.expect("migrations run");
    (pool, container)
}

/// Boots a fresh database with NO migrations applied.
async fn bare_pool() -> (PgPool, impl Sized) {
    let container = Postgres::default()
        .start()
        .await
        .expect("postgres container should start");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped postgres port");
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = adapter_pg::connect(&database_url)
        .await
        .expect("pool connects");
    (pool, container)
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
