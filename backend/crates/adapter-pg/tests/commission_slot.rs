//! Declared Slots over PostgreSQL (ZMVP-77), against a throwaway container:
//! `declare_slot` persists an ordinary component leaf in `commission_node`
//! **plus** its `commission_slot` satellite (required title, optional notes,
//! keyed by the slot node's id — the slot mirror of the Seat satellite ruling,
//! Gate A E20) in one transaction; the parent gates match the other tree
//! writes; and the satellite cascades away with its commission (ruling E35 —
//! what ZMVP-66's "gone entirely" relies on). Requires a container runtime
//! socket (DOCKER_HOST honored).

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            Commission, CommissionTitle, NewComponent, NewSlot, NodeId, NodeKind, SlotTitle,
        },
        did::Did,
        user::User,
    },
    ports::{CommissionStore, Database, ParentNodeNotFound, ParentNotASurface},
};
use serde_json::json;
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};

/// Boots a fresh database and runs migrations. The container is returned so the
/// caller keeps it alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
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
    adapter_pg::migrate(&pool).await.expect("migrations run");
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

/// Create a commission (which mints its root) in one committed unit of work,
/// returning `(the commission, its root node id)`.
async fn rooted_commission(pool: &PgPool, owner: &User, title: &str) -> (Commission, NodeId) {
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
    let root = PgCommissionStore::new(pool.clone())
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists")
        .root
        .id;
    (commission, root)
}

/// The satellite row as stored, or `None` — `(title, notes)`.
async fn slot_row(pool: &PgPool, node: NodeId) -> Option<(String, Option<String>)> {
    sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT title, notes FROM commission_slot WHERE node_id = $1",
    )
    .bind(*node)
    .fetch_optional(pool)
    .await
    .expect("query commission_slot")
}

// AC1/AC2 (pg) — declaring persists the component leaf AND its satellite in one
// unit: the node reads back as an ordinary Component (empty payload, owner's
// envelope, append order), the satellite carries title + notes (and None for
// omitted notes), and a commission holds zero, then several, Slots.
#[tokio::test]
async fn declare_slot_persists_the_leaf_and_its_satellite() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-owner").await;
    let (commission, root) = rooted_commission(&pool, &owner, "Two characters").await;

    let noted = NewSlot::under(
        commission.id,
        root,
        SlotTitle::try_new("The knight").expect("valid"),
        Some("full plate, no cape".to_string()),
        owner.id,
        Utc::now(),
    );
    let bare = NewSlot::under(
        commission.id,
        root,
        SlotTitle::try_new("The mage").expect("valid"),
        None,
        owner.id,
        Utc::now(),
    );
    let (noted_id, bare_id) = (noted.id, bare.id);

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        commissions.declare_slot(&noted).await.expect("first slot");
        commissions.declare_slot(&bare).await.expect("second slot");
    }
    uow.commit().await.expect("commit");

    let tree = PgCommissionStore::new(pool.clone())
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 2);
    assert_eq!(tree.root.children[0].id, noted_id, "append order holds");
    assert_eq!(tree.root.children[1].id, bare_id);
    for child in &tree.root.children {
        assert!(
            matches!(child.kind, NodeKind::Component),
            "a slot is an ordinary component leaf"
        );
        assert_eq!(child.created_by, owner.id);
        assert_eq!(child.payload, json!({}), "the substance is the satellite's");
        assert!(child.children.is_empty());
    }

    assert_eq!(
        slot_row(&pool, noted_id).await,
        Some((
            "The knight".to_string(),
            Some("full plate, no cape".to_string())
        )),
    );
    assert_eq!(
        slot_row(&pool, bare_id).await,
        Some(("The mage".to_string(), None)),
        "omitted notes store as NULL"
    );
}

// The parent gates match the other tree writes: absent and foreign parents are
// one indistinguishable ParentNodeNotFound, a component parent (a slot
// included) is ParentNotASurface — and no refused write leaves either row.
#[tokio::test]
async fn declare_slot_refuses_bad_parents_like_every_tree_write() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-gates").await;
    let (mine, my_root) = rooted_commission(&pool, &owner, "Mine").await;
    let other = provision(&pool, "did:plc:slot-other").await;
    let (_theirs, their_root) = rooted_commission(&pool, &other, "Theirs").await;

    let db = PgDatabase::new(pool.clone());

    // Seed a component to use as an illegal parent.
    let component = NewComponent::under(mine.id, my_root, json!({}), owner.id, Utc::now());
    let component_id = component.id;
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .add_component(&component)
        .await
        .expect("component");
    uow.commit().await.expect("commit");

    let title = || SlotTitle::try_new("The knight").expect("valid");

    // Fabricated parent.
    let fabricated = NewSlot::under(
        mine.id,
        NodeId::new(uuid::Uuid::now_v7()),
        title(),
        None,
        owner.id,
        Utc::now(),
    );
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .declare_slot(&fabricated)
        .await
        .expect_err("absent parent refuses");
    assert!(
        err.downcast_ref::<ParentNodeNotFound>().is_some(),
        "expected ParentNodeNotFound, got: {err:?}"
    );
    drop(uow);

    // A real surface — in someone else's tree.
    let cross = NewSlot::under(mine.id, their_root, title(), None, owner.id, Utc::now());
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .declare_slot(&cross)
        .await
        .expect_err("foreign parent refuses");
    assert!(
        err.downcast_ref::<ParentNodeNotFound>().is_some(),
        "a foreign-tree parent is indistinguishable from an absent one, got: {err:?}"
    );
    drop(uow);

    // A component parent.
    let nested = NewSlot::under(mine.id, component_id, title(), None, owner.id, Utc::now());
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .declare_slot(&nested)
        .await
        .expect_err("component parent refuses");
    assert!(
        err.downcast_ref::<ParentNotASurface>().is_some(),
        "expected ParentNotASurface, got: {err:?}"
    );
    drop(uow);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM commission_slot")
        .fetch_one(&pool)
        .await
        .expect("count slots");
    assert_eq!(count, 0, "no refused declaration left a satellite behind");
}

// Transactionality — node and satellite land (or vanish) together: a rolled-
// back unit leaves neither row.
#[tokio::test]
async fn a_rolled_back_declaration_leaves_neither_row() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-tx").await;
    let (commission, root) = rooted_commission(&pool, &owner, "Tx").await;

    let slot = NewSlot::under(
        commission.id,
        root,
        SlotTitle::try_new("Never lands").expect("valid"),
        None,
        owner.id,
        Utc::now(),
    );
    let slot_id = slot.id;

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions().declare_slot(&slot).await.expect("stage");
    uow.rollback().await.expect("rollback");

    assert!(slot_row(&pool, slot_id).await.is_none(), "no satellite row");
    let tree = PgCommissionStore::new(pool.clone())
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists");
    assert!(tree.root.children.is_empty(), "no node row");
}

// Ruling E35 — the satellite cascades away with its commission (both through
// its own commission FK and through the node's), so ZMVP-66's hard-delete
// sweeps declared Slots for free.
#[tokio::test]
async fn slots_cascade_away_with_their_commission() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-cascade").await;
    let (commission, root) = rooted_commission(&pool, &owner, "Doomed").await;

    let slot = NewSlot::under(
        commission.id,
        root,
        SlotTitle::try_new("Swept").expect("valid"),
        Some("goes with the ship".to_string()),
        owner.id,
        Utc::now(),
    );
    let slot_id = slot.id;
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .declare_slot(&slot)
        .await
        .expect("declare");
    uow.commit().await.expect("commit");
    assert!(slot_row(&pool, slot_id).await.is_some(), "satellite landed");

    // No delete port exists yet (ZMVP-66); exercise the schema's own cascade.
    sqlx::query("DELETE FROM commission WHERE id = $1")
        .bind(*commission.id)
        .execute(&pool)
        .await
        .expect("delete commission");

    assert!(
        slot_row(&pool, slot_id).await.is_none(),
        "the satellite cascades away with the commission"
    );
}
