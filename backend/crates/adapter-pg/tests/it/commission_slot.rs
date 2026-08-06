//! Declared Slots over PostgreSQL (ZMVP-77), against a throwaway container:
//! `declare_slots` contributes an ordinary element into `commission_element`
//! **plus** the Slot itself as its `commission_slot` satellite (required title,
//! optional notes, keyed by the carrying element's id — the Slot mirror of the
//! Seat satellite ruling, Gate A E20) in one transaction; the address gates
//! match every other element write (ZMVP-166); and the satellite cascades away
//! with its commission (ruling E35 — what ZMVP-66's "gone entirely" relies on).
//! Requires a container runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            Commission, CommissionId, CommissionTitle, ElementId, ElementType, NewSlot, SKELETON,
            SlotTitle, SurfaceAddress, SurfaceName, TabId,
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
        .provision(&Did::new(did.to_string()))
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
    let commission = Commission::create(
        title.parse::<CommissionTitle>().expect("valid title"),
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
    let address = address_of(pool, commission.id).await;
    (commission, address)
}

/// The commission's one skeleton address — the placeholder skeleton declares
/// exactly one tab holding exactly one surface, so this is unambiguous.
async fn address_of(pool: &PgPool, commission: CommissionId) -> SurfaceAddress {
    let composition = PgCommissionStore::new(pool.clone())
        .load_composition(commission)
        .await
        .expect("load composition")
        .expect("every commission has its tabs");
    SurfaceAddress::new(composition.tabs[0].id, only_surface())
}

/// The one surface the placeholder skeleton declares.
fn only_surface() -> SurfaceName {
    SKELETON[0].surfaces[0]
        .parse::<SurfaceName>()
        .expect("the skeleton declares valid labels")
}

/// The satellite row as stored, or `None` — `(title, notes)`.
async fn slot_row(pool: &PgPool, element: ElementId) -> Option<(String, Option<String>)> {
    sqlx::query_as::<_, (String, Option<String>)>(
        "SELECT title, notes FROM commission_slot WHERE element_id = $1",
    )
    .bind(*element)
    .fetch_optional(pool)
    .await
    .expect("query commission_slot")
}

// AC1/AC2 (pg) — declaring persists the carrying element AND its satellite in
// one unit: the element reads back typed `slot` with the empty payload, the
// owner's envelope, and append order; the satellite carries title + notes (and
// None for omitted notes); and a commission holds zero, then several, Slots.
#[tokio::test]
async fn declare_slot_persists_the_element_and_its_satellite() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-owner").await;
    let (commission, address) = composed_commission(&pool, &owner, "Two characters").await;

    let noted = NewSlot::contributed_at(
        commission.id,
        address.clone(),
        "The knight".parse::<SlotTitle>().expect("valid"),
        Some("full plate, no cape".to_string()),
        owner.id,
        Utc::now(),
    );
    let bare = NewSlot::contributed_at(
        commission.id,
        address.clone(),
        "The mage".parse::<SlotTitle>().expect("valid"),
        None,
        owner.id,
        Utc::now(),
    );
    let (noted_id, bare_id) = (noted.id, bare.id);

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    {
        let mut commissions = uow.commissions();
        commissions
            .declare_slots(&[noted, bare])
            .await
            .expect("the slot batch");
    }
    uow.commit().await.expect("commit");

    let composition = PgCommissionStore::new(pool.clone())
        .load_composition(commission.id)
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
            element.payload.as_value(),
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
    let title = || "The knight".parse::<SlotTitle>().expect("valid");
    let slot_at = |address: SurfaceAddress| {
        NewSlot::contributed_at(mine.id, address, title(), None, owner.id, Utc::now())
    };

    // A tab that exists nowhere.
    let fabricated = slot_at(SurfaceAddress::new(TabId::mint(), only_surface()));
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .declare_slots(&[fabricated])
        .await
        .expect_err("absent tab refuses");
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "expected UnknownTab, got: {err:?}"
    );
    drop(uow);

    // A real tab — in someone else's commission.
    let cross = slot_at(their_address);
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .declare_slots(&[cross])
        .await
        .expect_err("foreign tab refuses");
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "a foreign tab is indistinguishable from an absent one, got: {err:?}"
    );
    drop(uow);

    // A surface the skeleton does not declare.
    let invented = slot_at(SurfaceAddress::new(
        my_address.tab,
        "invented".parse::<SurfaceName>().expect("valid label"),
    ));
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .declare_slots(&[invented])
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

// Transactionality — element and satellite land (or vanish) together: a
// rolled-back unit leaves neither row.
#[tokio::test]
async fn a_rolled_back_declaration_leaves_neither_row() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-tx").await;
    let (commission, address) = composed_commission(&pool, &owner, "Tx").await;

    let slot = NewSlot::contributed_at(
        commission.id,
        address,
        "Never lands".parse::<SlotTitle>().expect("valid"),
        None,
        owner.id,
        Utc::now(),
    );
    let slot_id = slot.id;

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .declare_slots(&[slot])
        .await
        .expect("stage");
    uow.rollback().await.expect("rollback");

    assert!(slot_row(&pool, slot_id).await.is_none(), "no satellite row");
    let composition = PgCommissionStore::new(pool.clone())
        .load_composition(commission.id)
        .await
        .expect("load")
        .expect("composed");
    assert!(composition.elements.is_empty(), "no element row");
}

// Ruling E35 — the satellite cascades away with its commission (both through
// its own commission FK and through the element's), so ZMVP-66's hard-delete
// sweeps declared Slots for free.
#[tokio::test]
async fn slots_cascade_away_with_their_commission() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:slot-cascade").await;
    let (commission, address) = composed_commission(&pool, &owner, "Doomed").await;

    let slot = NewSlot::contributed_at(
        commission.id,
        address,
        "Swept".parse::<SlotTitle>().expect("valid"),
        Some("goes with the ship".to_string()),
        owner.id,
        Utc::now(),
    );
    let slot_id = slot.id;
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .declare_slots(&[slot])
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
