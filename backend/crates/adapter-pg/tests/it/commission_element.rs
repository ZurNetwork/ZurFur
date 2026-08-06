//! The flat commission composition over PostgreSQL (ZMVP-166; Flat Composition
//! DD `45514754`), against a throwaway container: every commission is born with
//! (or backfilled to) its skeleton tabs, the owner contributes elements into
//! declared surfaces in append order within a band, the whole composition loads
//! back, and removal renumbers the ordering group.
//!
//! **The structural tests are the point of this file.** Two composite foreign
//! keys carry the whole cross-commission property, one per level:
//!
//! - `(tab_id, commission_id) → commission_tab (id, commission_id)` makes an
//!   element citing another commission's tab unrepresentable;
//! - `(element key, commission_id) → commission_element (id, commission_id)`
//!   makes a Slot/Seat satellite claiming a different commission than its own
//!   element unrepresentable.
//!
//! Both are *unrepresentable* rather than merely refused, and only a test that
//! goes **around** the application gate — raw SQL, straight at the table — can
//! prove that. If either test ever has to be weakened, the security property
//! went with it.
//!
//! Requires a container runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            Band, Commission, CommissionId, CommissionTitle, ElementId, ElementPayload,
            ElementType, NewElement, SKELETON, SurfaceAddress, SurfaceName, TabId, VisibilityMode,
            declared_tabs,
        },
        did::Did,
        user::User,
    },
    ports::{CommissionStore, Database, ElementNotFound, UnknownSurface, UnknownTab},
};
use serde_json::json;

/// The ZMVP-166 migration (drop `commission_node`, create the flat tables, mint
/// the skeleton tabs), as sqlx numbers it. The backfill test runs everything
/// *before* this version, seeds pre-composition commissions, then lets the full
/// migrator catch up.
const FLAT_COMPOSITION_MIGRATION: i64 = 20260805234354;

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// A fresh, empty private database with NO migrations applied (the backfill
/// test drives the migrator itself).
async fn bare_pool() -> (PgPool, impl Sized) {
    let db = test_support::pg::bare_db().await;
    let pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    (pool, db)
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
/// work.
async fn create_commission(pool: &PgPool, owner: &User, title: &str) -> Commission {
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
    commission
}

/// The one surface the placeholder skeleton declares.
fn only_surface() -> SurfaceName {
    SKELETON[0].surfaces[0]
        .parse::<SurfaceName>()
        .expect("the skeleton declares valid labels")
}

/// The commission's one skeleton address — the placeholder skeleton declares
/// exactly one tab holding exactly one surface, so this is unambiguous.
async fn address_of(pool: &PgPool, commission: CommissionId) -> SurfaceAddress {
    let composition = PgCommissionStore::new(pool.clone())
        .load_composition(commission)
        .await
        .expect("load")
        .expect("a created commission always has its tabs");
    SurfaceAddress::new(composition.tabs[0].id, only_surface())
}

/// An untyped element at `address` — the shape most of these tests only need to
/// exist, so its type tag and payload carry no meaning.
fn element_at(commission: CommissionId, address: SurfaceAddress, owner: &User) -> NewElement {
    NewElement::contributed(
        commission,
        address,
        "note".parse::<ElementType>().expect("valid type"),
        ElementPayload::default(),
        owner.id,
        Utc::now(),
    )
}

/// Adds every element in one committed unit of work.
async fn add_elements(pool: &PgPool, elements: &[NewElement]) -> anyhow::Result<()> {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await?;
    {
        let mut commissions = uow.commissions();
        for element in elements {
            commissions.add_element(element).await?;
        }
    }
    uow.commit().await
}

/// Runs `remove_element` in its own committed unit of work.
async fn remove_element(
    pool: &PgPool,
    commission: CommissionId,
    element: ElementId,
) -> anyhow::Result<()> {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await?;
    uow.commissions()
        .remove_element(commission, element)
        .await?;
    uow.commit().await
}

/// The `(id, position)` pairs of one ordering group, ascending — read straight
/// off the table, so a test can assert the renumbering invariant (contiguous
/// from 0).
async fn group_positions(
    pool: &PgPool,
    commission: CommissionId,
    address: &SurfaceAddress,
) -> Vec<(uuid::Uuid, i32)> {
    sqlx::query_as::<_, (uuid::Uuid, i32)>(
        "SELECT id, position FROM commission_element
         WHERE commission_id = $1 AND tab_id = $2 AND surface = $3 AND band = $4
         ORDER BY position",
    )
    .bind(*commission)
    .bind(*address.tab)
    .bind(address.surface.as_str())
    .bind(Band::default().as_str())
    .fetch_all(pool)
    .await
    .expect("read group positions")
}

// ZMVP-166 (pg) — creating a commission mints its skeleton tabs in the same unit
// of work: the loaded composition holds exactly the declared tabs, every one
// born Total, and no elements. Nothing derives a tab's mode from
// `commission.visibility`: the commission is the formal root, gating OVER the
// composition rather than seeding it.
#[tokio::test]
async fn creating_a_commission_mints_its_skeleton_tabs() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:composition-owner").await;
    let commission = create_commission(&pool, &owner, "Composed").await;

    let store = PgCommissionStore::new(pool.clone());
    let composition = store
        .load_composition(commission.id)
        .await
        .expect("load")
        .expect("a created commission always has its tabs");

    let names: Vec<&str> = composition
        .tabs
        .iter()
        .map(|tab| tab.tab.as_str())
        .collect();
    let declared: Vec<String> = declared_tabs()
        .iter()
        .map(|tab| tab.as_str().to_owned())
        .collect();
    assert_eq!(
        names, declared,
        "exactly the code-declared skeleton, nothing more"
    );
    assert!(
        composition
            .tabs
            .iter()
            .all(|tab| tab.mode == VisibilityMode::Total),
        "every tab is born Total (the closed door)"
    );
    assert!(
        composition.elements.is_empty(),
        "a fresh commission is composed of nothing"
    );
    assert!(
        composition.surface_modes.is_empty(),
        "no surface has been widened, so no override row exists"
    );
}

// ZMVP-166 (pg) — elements append within their (tab, surface, band) group, are
// born Total, and their opaque payload round-trips as an equal JSON value.
#[tokio::test]
async fn add_element_appends_in_order_and_round_trips_its_payload() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:composer").await;
    let commission = create_commission(&pool, &owner, "Growing").await;
    let address = address_of(&pool, commission.id).await;

    let body = json!({
        "kind": "text",
        "body": "Reference: 三毛猫 🐾",
        "nested": { "list": [1, 2, 3], "flag": true, "nothing": null },
    });
    let first = NewElement::contributed(
        commission.id,
        address.clone(),
        "note".parse::<ElementType>().expect("valid"),
        ElementPayload::from(body.clone()),
        owner.id,
        Utc::now(),
    );
    let second = element_at(commission.id, address.clone(), &owner);
    let (first_id, second_id) = (first.id, second.id);
    add_elements(&pool, &[first, second]).await.expect("add");

    let composition = PgCommissionStore::new(pool.clone())
        .load_composition(commission.id)
        .await
        .expect("load")
        .expect("composed");
    assert_eq!(composition.elements.len(), 2);
    assert_eq!(composition.elements[0].id, first_id, "append order holds");
    assert_eq!(composition.elements[0].position, 0);
    assert_eq!(composition.elements[1].id, second_id);
    assert_eq!(composition.elements[1].position, 1);
    assert_eq!(
        composition.elements[0].payload.as_value(),
        &body,
        "the payload round-trips as an equal JSON value"
    );
    assert!(
        composition
            .elements
            .iter()
            .all(|element| element.mode == VisibilityMode::Total),
        "every element is born Total"
    );
    assert!(
        composition
            .elements
            .iter()
            .all(|element| element.band == Band::default()),
        "everything lands in the placeholder band"
    );
    assert_eq!(
        composition.effective_visibility_of(&composition.elements[0]),
        VisibilityMode::Total,
        "a commission nobody widened projects the closed door"
    );
}

// ZMVP-166 (pg) — THE SECURITY PROPERTY, proved structurally. An element citing
// another commission's tab is not merely refused by the application gate: the
// COMPOSITE foreign key makes the row unwritable, so even raw SQL that goes
// around every Rust check cannot land it. If this test has to be weakened, the
// property is gone.
#[tokio::test]
async fn a_cross_commission_tab_cite_is_unrepresentable_at_the_database() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:cross-prober").await;
    let victim = provision(&pool, "did:plc:cross-victim").await;
    let mine = create_commission(&pool, &owner, "Mine").await;
    let theirs = create_commission(&pool, &victim, "Theirs").await;
    let their_address = address_of(&pool, theirs.id).await;

    // Raw INSERT: my commission id, their tab id — no application code involved.
    let refused = sqlx::query(
        "INSERT INTO commission_element
             (id, commission_id, tab_id, surface, type, mode, band, position, created_by, created_at, payload)
         VALUES ($1, $2, $3, $4, 'note', 'total', 'body', 0, $5, now(), '{}'::jsonb)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(*mine.id)
    .bind(*their_address.tab)
    .bind(their_address.surface.as_str())
    .bind(*owner.id)
    .execute(&pool)
    .await;

    let err = refused.expect_err("the composite foreign key must refuse this row");
    let is_foreign_key_violation = err
        .as_database_error()
        .and_then(|db| db.code())
        .is_some_and(|code| code == "23503");
    assert!(
        is_foreign_key_violation,
        "expected a foreign-key violation (SQLSTATE 23503) — the STRUCTURAL refusal, \
         not an application check — got: {err:?}"
    );

    assert!(
        PgCommissionStore::new(pool.clone())
            .load_composition(mine.id)
            .await
            .expect("load")
            .expect("composed")
            .elements
            .is_empty(),
        "nothing landed"
    );
}

// ZMVP-166 (pg) — the tab must exist in THIS commission: a fabricated tab id and
// one belonging to another commission both refuse with UnknownTab (one
// indistinguishable answer — no probing other commissions), and neither write
// lands. This is the friendly answer sitting in front of the structural refusal
// above.
#[tokio::test]
async fn add_element_refuses_absent_and_foreign_tabs() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:tab-prober").await;
    let victim = provision(&pool, "did:plc:tab-victim").await;
    let mine = create_commission(&pool, &owner, "Mine").await;
    let theirs = create_commission(&pool, &victim, "Theirs").await;
    let their_address = address_of(&pool, theirs.id).await;

    let fabricated = element_at(
        mine.id,
        SurfaceAddress::new(TabId::mint(), only_surface()),
        &owner,
    );
    let err = add_elements(&pool, &[fabricated])
        .await
        .expect_err("a fabricated tab refuses");
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "expected UnknownTab, got: {err:?}"
    );

    let cross = element_at(mine.id, their_address, &owner);
    let err = add_elements(&pool, &[cross])
        .await
        .expect_err("a foreign tab refuses");
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "a foreign tab is indistinguishable from an absent one, got: {err:?}"
    );

    let store = PgCommissionStore::new(pool.clone());
    assert!(
        store
            .load_composition(mine.id)
            .await
            .expect("load")
            .expect("composed")
            .elements
            .is_empty(),
        "no refused write landed"
    );
    assert!(
        store
            .load_composition(theirs.id)
            .await
            .expect("load")
            .expect("composed")
            .elements
            .is_empty(),
        "and nothing leaked into the other commission"
    );
}

// ZMVP-166 (pg) — the surface must be one the CODE SKELETON declares. Surfaces
// have no rows, so the const is the only authority and an unrecognized name is
// refused, never created (fail-closed) — the same const adapter-mem consults.
#[tokio::test]
async fn add_element_refuses_an_undeclared_surface() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:surface-prober").await;
    let commission = create_commission(&pool, &owner, "Fail-closed").await;
    let address = address_of(&pool, commission.id).await;

    let invented = element_at(
        commission.id,
        SurfaceAddress::new(
            address.tab,
            "invented".parse::<SurfaceName>().expect("valid label"),
        ),
        &owner,
    );
    let err = add_elements(&pool, &[invented])
        .await
        .expect_err("an undeclared surface refuses");
    assert!(
        err.downcast_ref::<UnknownSurface>().is_some(),
        "expected UnknownSurface, got: {err:?}"
    );

    assert!(
        PgCommissionStore::new(pool.clone())
            .load_composition(commission.id)
            .await
            .expect("load")
            .expect("composed")
            .elements
            .is_empty(),
        "nothing landed, and no surface was invented"
    );
}

// ZMVP-166 (pg) — the skeleton check is on the PAIR. A surface that is real
// under its own tab, addressed under a DIFFERENT tab of the same commission, is
// refused with UnknownSurface: the pair is not declared. Not UnknownTab — the
// tab is genuinely this commission's. Same answer adapter-mem gives, from the
// same const.
#[tokio::test]
async fn add_element_refuses_a_real_surface_under_the_wrong_tab() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:wrong-addresser").await;
    let commission = create_commission(&pool, &owner, "Wrongly addressed").await;

    // A real tab row of THIS commission under a name the skeleton does not pair
    // with the surface below. The placeholder skeleton declares a single tab, so
    // the shape has to be planted with raw SQL; ZMVP-171's real, multi-tab
    // skeleton makes it an ordinary address.
    let other_tab = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO commission_tab (id, commission_id, tab) VALUES ($1, $2, 'other')")
        .bind(other_tab)
        .bind(*commission.id)
        .execute(&pool)
        .await
        .expect("plant a second tab row");

    let wrongly_addressed = element_at(
        commission.id,
        SurfaceAddress::new(TabId::new(other_tab), only_surface()),
        &owner,
    );
    let err = add_elements(&pool, &[wrongly_addressed])
        .await
        .expect_err("an undeclared (tab, surface) pair refuses");
    assert!(
        err.downcast_ref::<UnknownSurface>().is_some(),
        "the pair is undeclared — UnknownSurface, not UnknownTab (the tab is real), \
         got: {err:?}"
    );

    assert!(
        PgCommissionStore::new(pool.clone())
            .load_composition(commission.id)
            .await
            .expect("load")
            .expect("composed")
            .elements
            .is_empty(),
        "nothing landed under a place the skeleton never described"
    );
}

// ZMVP-166 (pg) — the gate ORDER, pinned: the pair check needs the tab's
// declared name, so the tab is resolved FIRST and an address wrong in both ways
// answers UnknownTab. adapter-mem pins the same case; if the two orders ever
// diverge, one request gets two answers depending on which store is live.
#[tokio::test]
async fn an_address_wrong_in_both_ways_answers_unknown_tab() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:doubly-wrong").await;
    let commission = create_commission(&pool, &owner, "Doubly wrong").await;

    let doubly_wrong = element_at(
        commission.id,
        SurfaceAddress::new(
            TabId::mint(),
            "invented".parse::<SurfaceName>().expect("valid label"),
        ),
        &owner,
    );
    let err = add_elements(&pool, &[doubly_wrong])
        .await
        .expect_err("a fabricated tab with an undeclared surface refuses");
    assert!(
        err.downcast_ref::<UnknownTab>().is_some(),
        "the tab is resolved first, so UnknownTab wins over UnknownSurface, got: {err:?}"
    );
}

// ZMVP-166 (pg) — THE SATELLITE HALF of the same structural property, proved the
// same way. A Slot or Seat satellite hangs off an element's id AND carries its
// own `commission_id`, which is what the by-commission reads filter on — so a
// desynced pair would list one commission's Seat among another's, past every
// application check, and project it under the wrong commission's visibility.
// The composite foreign key `(element key, commission_id) -> commission_element
// (id, commission_id)` makes that row unwritable even from raw SQL. If this test
// has to be weakened, the property is gone.
#[tokio::test]
async fn a_satellite_claiming_another_commission_is_unrepresentable_at_the_database() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:satellite-owner").await;
    let victim = provision(&pool, "did:plc:satellite-victim").await;
    let mine = create_commission(&pool, &owner, "Mine").await;
    let theirs = create_commission(&pool, &victim, "Theirs").await;

    // One ordinary element, legitimately in MY commission.
    let my_address = address_of(&pool, mine.id).await;
    let element = element_at(mine.id, my_address, &owner);
    let element_id = element.id;
    add_elements(&pool, &[element]).await.expect("add");

    // Raw INSERT: my element's id, THEIR commission id. Their commission exists,
    // so the satellite's own plain `commission_id` foreign key is satisfied —
    // only the composite key can catch this.
    let seat = sqlx::query(
        "INSERT INTO commission_seat (id, commission_id, kind) VALUES ($1, $2, 'Creator')",
    )
    .bind(*element_id)
    .bind(*theirs.id)
    .execute(&pool)
    .await;
    assert_satellite_desync_refused(seat, "commission_seat");

    let slot = sqlx::query(
        "INSERT INTO commission_slot (element_id, commission_id, title)
         VALUES ($1, $2, 'Smuggled')",
    )
    .bind(*element_id)
    .bind(*theirs.id)
    .execute(&pool)
    .await;
    assert_satellite_desync_refused(slot, "commission_slot");

    let seats = PgCommissionStore::new(pool.clone())
        .seats(theirs.id)
        .await
        .expect("read seats");
    assert!(
        seats.is_empty(),
        "nothing was smuggled into the other commission's seats"
    );
}

/// A satellite insert whose `commission_id` disagrees with its element's must be
/// refused **by the database** (SQLSTATE 23503, foreign-key violation) — a
/// structural refusal, not an application check.
fn assert_satellite_desync_refused(
    result: Result<sqlx::postgres::PgQueryResult, sqlx::Error>,
    table: &str,
) {
    let err = result.expect_err("the composite foreign key must refuse this row");
    let is_foreign_key_violation = err
        .as_database_error()
        .and_then(|db| db.code())
        .is_some_and(|code| code == "23503");
    assert!(
        is_foreign_key_violation,
        "{table}: expected a foreign-key violation (SQLSTATE 23503) — the STRUCTURAL \
         refusal, not an application check — got: {err:?}"
    );
}

// ZMVP-166 (pg) — ordering is UNIQUE within (commission, tab, surface, band):
// a duplicate position is unwritable even by raw SQL, and a removal renumbers
// the survivors contiguously from 0 in the same transaction (the UNIQUE being
// DEFERRABLE is what lets the renumbering UPDATE pass through its intermediate
// collisions).
#[tokio::test]
async fn positions_are_unique_within_the_group_and_renumber_on_removal() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:orderer").await;
    let commission = create_commission(&pool, &owner, "Ordered").await;
    let address = address_of(&pool, commission.id).await;

    let first = element_at(commission.id, address.clone(), &owner);
    let doomed = element_at(commission.id, address.clone(), &owner);
    let last = element_at(commission.id, address.clone(), &owner);
    let (first_id, doomed_id, last_id) = (first.id, doomed.id, last.id);
    add_elements(&pool, &[first, doomed, last])
        .await
        .expect("add");
    assert_eq!(
        group_positions(&pool, commission.id, &address).await,
        vec![(*first_id, 0), (*doomed_id, 1), (*last_id, 2)],
    );

    // A duplicate position is unwritable, even going around the store.
    let collision = sqlx::query(
        "INSERT INTO commission_element
             (id, commission_id, tab_id, surface, type, mode, band, position, created_by, created_at, payload)
         VALUES ($1, $2, $3, $4, 'note', 'total', 'body', 0, $5, now(), '{}'::jsonb)",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(*commission.id)
    .bind(*address.tab)
    .bind(address.surface.as_str())
    .bind(*owner.id)
    .execute(&pool)
    .await;
    let err = collision.expect_err("a duplicate position must be refused");
    let is_unique_violation = err
        .as_database_error()
        .and_then(|db| db.code())
        .is_some_and(|code| code == "23505");
    assert!(
        is_unique_violation,
        "expected a unique violation (SQLSTATE 23505), got: {err:?}"
    );

    remove_element(&pool, commission.id, doomed_id)
        .await
        .expect("removal succeeds");
    assert_eq!(
        group_positions(&pool, commission.id, &address).await,
        vec![(*first_id, 0), (*last_id, 1)],
        "the survivors renumber contiguously from 0, order preserved"
    );
}

// ZMVP-166 (pg) — THE REMOVAL TAKES THE TAB LOCK, and this is what proves it
// rather than trusting the doc comment.
//
// Why it matters: an append computes its `position` as `max + 1` under that same
// lock. Before this fix, only the add path locked — so a removal's renumbering
// UPDATE could interleave with an append's max-subquery-then-INSERT, land two
// rows on one position, and abort the whole unit on the deferred UNIQUE at
// commit. A legitimate request failing because another legitimate request
// happened at the wrong moment.
//
// The proof is deterministic, not timing luck: another transaction holds the tab
// row's lock, so Postgres MUST block a second `SELECT … FOR UPDATE` on it. The
// removal therefore cannot finish while the lock is held — and finishes at once
// after it is released. Without the lock in `remove_element`, the removal would
// complete inside milliseconds and the first half of this test would fail.
#[tokio::test]
async fn remove_element_blocks_on_the_tab_lock_the_add_path_takes() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:lock-contender").await;
    let commission = create_commission(&pool, &owner, "Contended").await;
    let address = address_of(&pool, commission.id).await;

    let doomed = element_at(commission.id, address.clone(), &owner);
    let doomed_id = doomed.id;
    add_elements(&pool, &[doomed]).await.expect("add");

    // A rival transaction takes the very lock the add path takes, and holds it.
    let mut rival = pool.begin().await.expect("rival begins");
    sqlx::query("SELECT id FROM commission_tab WHERE id = $1 FOR UPDATE")
        .bind(*address.tab)
        .fetch_one(&mut *rival)
        .await
        .expect("the rival holds the tab lock");

    // The removal must not get through while the lock is held.
    let blocked = tokio::time::timeout(
        std::time::Duration::from_millis(750),
        remove_element(&pool, commission.id, doomed_id),
    )
    .await;
    assert!(
        blocked.is_err(),
        "remove_element completed while another transaction held the tab lock — \
         it is NOT taking the lock, and a concurrent append can still collide on \
         `position` (got: {blocked:?})"
    );

    // Release it, and the same removal goes straight through.
    rival.rollback().await.expect("rival releases the lock");
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        remove_element(&pool, commission.id, doomed_id),
    )
    .await
    .expect("the removal proceeds once the lock is free")
    .expect("removal succeeds");

    assert!(
        group_positions(&pool, commission.id, &address)
            .await
            .is_empty(),
        "the element is gone"
    );
}

// ZMVP-166 (pg) — a fabricated element id and one belonging to another
// commission both refuse removal with ElementNotFound (indistinguishably), and
// nothing is removed anywhere. There is no protected-element arm to leak past:
// tabs and surfaces are skeleton, not elements.
#[tokio::test]
async fn remove_refuses_absent_and_foreign_elements() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:removal-prober").await;
    let victim = provision(&pool, "did:plc:removal-victim").await;
    let mine = create_commission(&pool, &owner, "Mine").await;
    let theirs = create_commission(&pool, &victim, "Theirs").await;

    let their_address = address_of(&pool, theirs.id).await;
    let theirs_element = element_at(theirs.id, their_address, &victim);
    let their_element_id = theirs_element.id;
    add_elements(&pool, &[theirs_element]).await.expect("add");

    let err = remove_element(&pool, mine.id, ElementId::new(uuid::Uuid::now_v7()))
        .await
        .expect_err("a fabricated element refuses");
    assert!(
        err.downcast_ref::<ElementNotFound>().is_some(),
        "expected ElementNotFound, got: {err:?}"
    );

    let err = remove_element(&pool, mine.id, their_element_id)
        .await
        .expect_err("a foreign element refuses");
    assert!(
        err.downcast_ref::<ElementNotFound>().is_some(),
        "a foreign element is indistinguishable from an absent one, got: {err:?}"
    );

    assert_eq!(
        PgCommissionStore::new(pool.clone())
            .load_composition(theirs.id)
            .await
            .expect("load")
            .expect("composed")
            .elements
            .len(),
        1,
        "the other commission's composition is untouched"
    );
}

// ZMVP-166 (pg) — `load_composition` answers None only for a commission that does
// not exist; a created-but-empty one is Some with its tabs and no elements
// (empty is not absent).
#[tokio::test]
async fn load_composition_distinguishes_absent_from_empty() {
    let (pool, _container) = fresh_pool().await;
    let store = PgCommissionStore::new(pool.clone());
    assert!(
        store
            .load_composition(CommissionId::new(uuid::Uuid::now_v7()))
            .await
            .expect("load")
            .is_none(),
        "an unknown commission composes to None"
    );

    let owner = provision(&pool, "did:plc:empty-composer").await;
    let commission = create_commission(&pool, &owner, "Empty").await;
    let composition = store
        .load_composition(commission.id)
        .await
        .expect("load")
        .expect("an existing commission composes to Some, however empty");
    assert!(!composition.tabs.is_empty(), "its tabs are there");
    assert!(composition.elements.is_empty(), "empty is not absent");
}

// ZMVP-166, the retroactive half — commissions created BEFORE the flat model
// get their skeleton tabs backfilled by the migration, born Total **regardless
// of the commission's visibility**: the closed door, not a mapping. (The retired
// tree's migration DID map `visibility` onto its root's mode, because the root
// was the only home the mode had; the flat model gives every term its own, so
// copying the column in would widen composition nobody chose to widen.)
//
// AND the membership sweep (Engineer ruling 2026-08-05, security finding F1):
// the migration drops every Seat, so it also drops the non-owner participant
// rows those Seats justified. A participant row is the key to the closed door —
// leaving one behind after its seat is gone would let a User keep reading a
// composition they hold no position in. The owner's permanent floor row is the
// exception and survives.
#[tokio::test]
async fn the_migration_backfills_skeleton_tabs_for_pre_composition_commissions() {
    let (pool, _container) = bare_pool().await;

    // Run every migration BEFORE the flat-composition one.
    let mut pre_flat = adapter_pg::migrator();
    let migrations: Vec<_> = pre_flat
        .migrations
        .iter()
        .filter(|m| m.version < FLAT_COMPOSITION_MIGRATION)
        .cloned()
        .collect();
    assert!(
        !migrations.is_empty() && migrations.len() < pre_flat.migrations.len(),
        "the version constant matches an embedded migration"
    );
    pre_flat.migrations = migrations.into();
    pre_flat.run(&pool).await.expect("pre-flat migrations run");

    // Seed a pre-composition world: an owner and one commission per visibility
    // value. The rows go in directly because the store's `create` would already
    // want the `commission_tab` table this migration has not yet created.
    let owner = provision(&pool, "did:plc:pre-flat-owner").await;
    // A User who was SEATED on the old model: membership justified by a Seat the
    // migration is about to drop.
    let seated = provision(&pool, "did:plc:pre-flat-seated").await;
    let mut seeded = Vec::new();
    for visibility in ["private", "listed", "public"] {
        let id = uuid::Uuid::now_v7();
        sqlx::query(
            "INSERT INTO commission (id, title, owner_id, lifecycle, visibility, created_at)
             VALUES ($1, $2, $3, 'draft', $4, $5)",
        )
        .bind(id)
        .bind(format!("Pre-flat {visibility}"))
        .bind(*owner.id)
        .bind(visibility)
        .bind(Utc::now())
        .execute(&pool)
        .await
        .expect("seed pre-composition commission");

        // Both memberships, as the old model would have held them: the owner's
        // permanent floor row and one seated non-owner.
        for user in [owner.id, seated.id] {
            sqlx::query(
                "INSERT INTO commission_participant (commission_id, user_id, created_at)
                 VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(*user)
            .bind(Utc::now())
            .execute(&pool)
            .await
            .expect("seed pre-composition participant");
        }
        seeded.push((CommissionId::new(id), visibility));
    }

    // The world catches up: the remaining migrations (including the backfill) run.
    adapter_pg::migrate(&pool).await.expect("catch-up migrates");

    let store = PgCommissionStore::new(pool.clone());
    for (id, visibility) in seeded {
        let composition = store
            .load_composition(id)
            .await
            .expect("load")
            .expect("the backfill minted its tabs");
        let names: Vec<&str> = composition
            .tabs
            .iter()
            .map(|tab| tab.tab.as_str())
            .collect();
        let declared: Vec<String> = declared_tabs()
            .iter()
            .map(|tab| tab.as_str().to_owned())
            .collect();
        assert_eq!(names, declared, "the skeleton, backfilled");
        assert!(
            composition
                .tabs
                .iter()
                .all(|tab| tab.mode == VisibilityMode::Total),
            "a {visibility} commission's backfilled tabs are STILL Total — \
             visibility is the outer gate, never copied into the composition"
        );
        assert!(composition.elements.is_empty());

        // The membership sweep: only the owner's floor row survives. The seated
        // User's row died with the Seat that justified it.
        let members: Vec<uuid::Uuid> = sqlx::query_scalar(
            "SELECT user_id FROM commission_participant WHERE commission_id = $1",
        )
        .bind(*id)
        .fetch_all(&pool)
        .await
        .expect("read the surviving membership");
        assert_eq!(
            members,
            vec![*owner.id],
            "only the owner's permanent floor row survives the migration — a seated \
             User whose Seat was dropped must not keep the key to the closed door"
        );
        assert!(
            !store
                .is_participant(id, seated.id)
                .await
                .expect("is_participant"),
            "and the closed-door gate agrees: the ex-seated User is no longer a Participant"
        );
    }

    // And the tree it replaced is gone.
    let node_table_exists: bool =
        sqlx::query_scalar("SELECT to_regclass('commission_node') IS NOT NULL")
            .fetch_one(&pool)
            .await
            .expect("probe for the retired table");
    assert!(
        !node_table_exists,
        "the recursive commission_node tree is dropped, not left beside the flat model"
    );
}
