//! Declared Slots over PostgreSQL, against a throwaway container:
//! `declare_slot` contributes an ordinary element into `commission_element`
//! **plus** the Slot itself as its `commission_slot` satellite (optional notes, keyed by the
//! carrying element's id, bound to its node, whose name is its title — the same satellite
//! pattern the Seat table uses) in one transaction; the address gates match every other
//! element write; the satellite insert binds only the Slot's own node under its own
//! commission's `slots` entry; and the satellite cascades away with its commission (what the
//! commission's "gone entirely" hard-delete relies on). Requires a container runtime socket
//! (DOCKER_HOST honored).

use crate::seed::{self, declare_committed, declare_in, owner_on, slot_node, slots_entry_in};
use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            Commission, ElementId, ElementType, NewSlot, SKELETON, SlotBirth, SlotDeclaration,
            SurfaceAddress, SurfaceName, TabId,
        },
        did::Did,
        user::User,
    },
    ports::{CommissionStore, Database, UnknownSurface, UnknownTab},
};
use serde_json::json;

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// Recognize a visitor in its own committed unit of work
/// (`commission_element.created_by` references `users(id)`).
async fn provision(pool: &PgPool, did: &str) -> User {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::from(did.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// Create a commission (which mints its skeleton tabs) in one committed unit of
/// work, returning `(the commission, its one skeleton address)`.
async fn composed_commission(
    pool: &PgPool,
    owner: &User,
    title: &str,
) -> (Commission, SurfaceAddress) {
    let commission = seed::seed_stored(pool, &owner.id, title).await;
    let address = seed::only_address(pool, commission.id)
        .await
        .expect("the commission's address");
    (commission, address)
}

/// The one surface the placeholder skeleton declares.
fn only_surface() -> SurfaceName {
    SKELETON[0].surfaces[0]
        .parse::<SurfaceName>()
        .expect("the skeleton declares valid labels")
}

/// The satellite row as stored, or `None` — `(its node's name, notes)`.
async fn slot_row(pool: &PgPool, element: ElementId) -> Option<(String, Option<String>)> {
    sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT n.name, s.notes FROM commission_slot s JOIN node n ON n.id = s.node_id
         WHERE s.element_id = $1",
    )
    .bind(uuid::Uuid::from(element))
    .fetch_optional(pool)
    .await
    .expect("query commission_slot")
}

/// A fresh element id, as a declare mints one.
fn fresh_element(address: &SurfaceAddress) -> ElementId {
    let slot = NewSlot {
        address: address.clone(),
        notes: None,
    };
    SlotDeclaration::minted(slot).element
}

// AC1/AC2 (pg) — declaring persists the carrying element AND its satellite in
// one unit: the element reads back typed `slot` with the empty payload, the
// owner's envelope, and append order; the satellite carries notes (and None for omitted notes)
// and its node's name as its title; and a commission holds zero, then several, Slots.
#[tokio::test]
async fn declare_slot_persists_the_element_and_its_satellite() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-owner").await;
    let (commission, address) = composed_commission(&pool, &owner, "Two characters").await;

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let noted_id = declare_in(
        &mut *uow,
        commission.id,
        &owner.id,
        &address,
        "The knight",
        Some("full plate, no cape"),
    )
    .await
    .expect("the knight");
    let bare_id = declare_in(
        &mut *uow,
        commission.id,
        &owner.id,
        &address,
        "The mage",
        None,
    )
    .await
    .expect("the mage");
    uow.commit().await.expect("commit");

    let composition = PgCommissionStore::new(pool.clone())
        .load_composition(&commission.id)
        .await
        .expect("load")
        .expect("composed");
    assert_eq!(composition.elements.len(), 2);
    assert_eq!(composition.elements[0].id, noted_id, "append order holds");
    assert_eq!(composition.elements[1].id, bare_id);
    for element in &composition.elements {
        assert_eq!(
            element.element_type,
            ElementType::slot(),
            "a slot's carrier is an ordinary element, typed `slot`"
        );
        assert_eq!(element.created_by, owner.id);
        assert_eq!(
            element.payload.as_ref(),
            &json!({}),
            "the substance is the satellite's"
        );
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

// The address gates match every other element write: an absent tab and a
// foreign one are one indistinguishable UnknownTab, an undeclared surface is
// UnknownSurface — and no refused write leaves either row.
#[tokio::test]
async fn declare_slot_refuses_bad_addresses_like_every_element_write() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-gates").await;
    let (mine, my_address) = composed_commission(&pool, &owner, "Mine").await;
    let other = provision(&pool, "did:plc:slot-other").await;
    let (_theirs, their_address) = composed_commission(&pool, &other, "Theirs").await;

    let db = PgDatabase::new(pool.clone());
    let fabricated = SurfaceAddress::new(TabId::mint(), only_surface());
    let invented = SurfaceAddress::new(
        my_address.tab,
        "invented".parse::<SurfaceName>().expect("valid label"),
    );

    let mut uow = db.begin().await.expect("begin");
    let err = declare_in(&mut *uow, mine.id, &owner.id, &fabricated, "Knight", None)
        .await
        .expect_err("absent tab refuses");
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "expected UnknownTab, got: {err:?}"
    );
    drop(uow);

    let mut uow = db.begin().await.expect("begin");
    let err = declare_in(
        &mut *uow,
        mine.id,
        &owner.id,
        &their_address,
        "Knight",
        None,
    )
    .await
    .expect_err("foreign tab refuses");
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "a foreign tab is indistinguishable from an absent one, got: {err:?}"
    );
    drop(uow);

    let mut uow = db.begin().await.expect("begin");
    let err = declare_in(&mut *uow, mine.id, &owner.id, &invented, "Knight", None)
        .await
        .expect_err("undeclared surface refuses");
    assert!(
        err.downcast_ref::<UnknownSurface>().is_some(),
        "expected UnknownSurface, got: {err:?}"
    );
    drop(uow);

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM commission_slot")
        .fetch_one(&pool)
        .await
        .expect("count slots");
    assert_eq!(count, 0, "no refused declaration left a satellite behind");
}

// The satellite insert binds only the Slot's own node: a declare on this commission whose node
// is a slot node under another commission's `slots` entry inserts no row, and the declare errs
// and ends its unit, whatever the commit tries after.
#[tokio::test]
async fn a_satellite_insert_on_another_commission_s_slot_node_fails_at_the_statement() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-foreign-node").await;
    let (mine, my_address) = composed_commission(&pool, &owner, "Mine").await;
    let (theirs, _) = composed_commission(&pool, &owner, "Theirs").await;
    let element = fresh_element(&my_address);

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let their_slots = slots_entry_in(&mut *uow, theirs.id)
        .await
        .expect("their slots entry");
    let foreign = slot_node(their_slots, element, "Elsewhere").expect("a slot node");
    uow.nodes()
        .create(&foreign)
        .await
        .expect("a slot node under their slots entry");
    let birth = SlotBirth {
        element,
        node: foreign.id,
        created_by: owner.id.clone(),
        created_at: Utc::now(),
    };
    let slot = NewSlot {
        address: my_address,
        notes: None,
    };
    let declared = uow
        .commissions()
        .declare_slot(&owner_on(mine.id), &birth, &slot)
        .await;
    let committed = uow.commit().await;

    let refusal = declared.expect_err("the satellite insert matches no node");
    assert!(
        refusal.to_string().contains("not the slot's own"),
        "{refusal}"
    );
    assert!(committed.is_err(), "the unit ends with the refusal");
    assert!(slot_row(&pool, element).await.is_none(), "no satellite row");
    let composition = PgCommissionStore::new(pool.clone())
        .load_composition(&mine.id)
        .await
        .expect("load")
        .expect("composed");
    assert!(composition.elements.is_empty(), "no element row");
}

// A slot node no Slot points at is refused at commit, and nothing of its unit is kept.
#[tokio::test]
async fn a_slot_node_without_its_slot_is_refused_at_commit() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-phantom").await;
    let (commission, address) = composed_commission(&pool, &owner, "Phantom").await;
    let element = fresh_element(&address);

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let slots = slots_entry_in(&mut *uow, commission.id)
        .await
        .expect("the slots entry");
    let phantom = slot_node(slots, element, "Phantom").expect("a slot node");
    uow.nodes()
        .create(&phantom)
        .await
        .expect("a slot node on the unit");
    let committed = uow.commit().await;

    let refusal = committed.expect_err("a slot node without its slot is refused at commit");
    assert!(
        format!("{refusal:?}").contains("has no slot"),
        "{refusal:?}"
    );
    let kept: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM node WHERE type = 'slot'")
        .fetch_one(&pool)
        .await
        .expect("count slot nodes");
    assert_eq!(kept, 0, "the slot node is not kept");
}

// Transactionality — element and satellite land (or vanish) together: a
// rolled-back unit leaves neither row.
#[tokio::test]
async fn a_rolled_back_declaration_leaves_neither_row() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-tx").await;
    let (commission, address) = composed_commission(&pool, &owner, "Tx").await;

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let slot_id = declare_in(
        &mut *uow,
        commission.id,
        &owner.id,
        &address,
        "Never lands",
        None,
    )
    .await
    .expect("stage");
    uow.rollback().await.expect("rollback");

    assert!(slot_row(&pool, slot_id).await.is_none(), "no satellite row");
    let composition = PgCommissionStore::new(pool.clone())
        .load_composition(&commission.id)
        .await
        .expect("load")
        .expect("composed");
    assert!(composition.elements.is_empty(), "no element row");
}

// Ruling E35 — the satellite cascades away with its commission (both through
// its own commission FK and through the element's), so ZMVP-66's hard-delete
// sweeps declared Slots for free, their nodes with them.
#[tokio::test]
async fn slots_cascade_away_with_their_commission() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-cascade").await;
    let (commission, address) = composed_commission(&pool, &owner, "Doomed").await;
    let slot_id = declare_committed(
        &pool,
        commission.id,
        &owner.id,
        &address,
        "Swept",
        Some("goes with the ship"),
    )
    .await;
    assert!(slot_row(&pool, slot_id).await.is_some(), "satellite landed");

    // No delete port exists yet (ZMVP-66); exercise the schema's own cascade.
    sqlx::query("DELETE FROM commission WHERE id = $1")
        .bind(uuid::Uuid::from(commission.id))
        .execute(&pool)
        .await
        .expect("delete commission");

    assert!(
        slot_row(&pool, slot_id).await.is_none(),
        "the satellite cascades away with the commission"
    );
    let slot_nodes: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM node WHERE type = 'slot'")
        .fetch_one(&pool)
        .await
        .expect("count slot nodes");
    assert_eq!(slot_nodes, 0, "the Slot's node went with it");
}

// The Slot list reads each Slot's title from its node: a rename of the node is the new title.
#[tokio::test]
async fn a_slot_s_title_is_read_from_its_node() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-node-title").await;
    let (commission, address) = composed_commission(&pool, &owner, "Renamed").await;
    let slot_id =
        declare_committed(&pool, commission.id, &owner.id, &address, "Before", None).await;

    sqlx::query("UPDATE node SET name = 'After' WHERE segment = $1 AND type = 'slot'")
        .bind(slot_id.to_string())
        .execute(&pool)
        .await
        .expect("rename the slot node");
    let listed = PgCommissionStore::new(pool.clone())
        .slots(&commission.id)
        .await
        .expect("the slots read");

    let titles: Vec<&str> = listed.iter().map(|slot| slot.title.as_str()).collect();
    assert_eq!(titles, ["After"]);
}
