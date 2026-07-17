//! Participants and Seats over PostgreSQL (ZMVP-76), against a throwaway
//! container: creating a commission persists its owner's participant row (and
//! the migration backfills one for commissions that predate the table);
//! `is_participant` reads the membership record, not the owner column; the
//! owner's row is the irremovable permanent floor (while the commission-delete
//! cascade still sweeps everything); a re-add for an already-seated pair is a
//! silent no-op that preserves the original created_at (ZMVP-140, ahead of
//! ZMVP-79's seat acceptance re-adding an existing participant); and a
//! declared Seat lands as one component node plus its interpreted satellite
//! sharing the id, read back through `seats()`. Requires a container runtime
//! socket (DOCKER_HOST honored).

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            Commission, CommissionId, CommissionTitle, NewSeat, NodeId, NodeKind, SeatKind,
            SeatLink, SeatPrompt,
        },
        did::Did,
        user::{User, UserId},
    },
    ports::{CommissionStore, Database, ParentNodeNotFound},
};

/// The ZMVP-76 migration (create `commission_participant` + owner backfill +
/// `commission_seat`), as sqlx numbers it. The backfill test runs everything
/// *before* this version, seeds pre-membership commissions, then lets the full
/// migrator catch up.
const PARTICIPANT_SEAT_MIGRATION: i64 = 20260706100000;

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
/// (`commission_participant.user_id` references `users(id)`).
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

/// Create a commission (which mints its root AND its owner's participant row)
/// in one committed unit of work.
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

/// The commission's root node id.
async fn root_of(pool: &PgPool, commission: CommissionId) -> NodeId {
    PgCommissionStore::new(pool.clone())
        .load_tree(commission)
        .await
        .expect("load tree")
        .expect("every commission has a tree")
        .root
        .id
}

/// The participant rows for a commission, as raw `(user_id)` values.
async fn participant_rows(pool: &PgPool, commission: CommissionId) -> Vec<uuid::Uuid> {
    sqlx::query_scalar::<_, uuid::Uuid>(
        "SELECT user_id FROM commission_participant WHERE commission_id = $1",
    )
    .bind(*commission)
    .fetch_all(pool)
    .await
    .expect("scan commission_participant")
}

// Ruling B2 (pg) — creating a commission persists its owner's participant row
// in the same transaction, stamped with the commission's creation instant; the
// predicate answers from that record.
#[tokio::test]
async fn creating_a_commission_persists_its_owners_participant_row() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:seat-owner").await;
    let commission = create_commission(&pool, &owner, "Membered").await;

    assert_eq!(
        participant_rows(&pool, commission.id).await,
        vec![*owner.id],
        "exactly the owner's membership row is born with the commission"
    );

    let store = PgCommissionStore::new(pool.clone());
    assert!(
        store
            .is_participant(commission.id, owner.id)
            .await
            .expect("predicate"),
        "the owner is a Participant"
    );
    let stranger = provision(&pool, "did:plc:stranger").await;
    assert!(
        !store
            .is_participant(commission.id, stranger.id)
            .await
            .expect("predicate"),
        "a stranger is not"
    );
}

// Ruling B2 (pg) — is_participant reads the membership TABLE, not the owner
// column: a directly inserted membership row for a non-owner (the shape
// ZMVP-79's seated arm will write) already counts.
#[tokio::test]
async fn is_participant_reads_the_membership_record_not_the_owner_column() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:seat-owner").await;
    let seated = provision(&pool, "did:plc:seated-later").await;
    let commission = create_commission(&pool, &owner, "Seated later").await;

    let store = PgCommissionStore::new(pool.clone());
    assert!(
        !store
            .is_participant(commission.id, seated.id)
            .await
            .expect("predicate"),
        "no membership row yet"
    );

    sqlx::query(
        "INSERT INTO commission_participant (commission_id, user_id, created_at)
         VALUES ($1, $2, $3)",
    )
    .bind(*commission.id)
    .bind(*seated.id)
    .bind(Utc::now())
    .execute(&pool)
    .await
    .expect("seed a seated membership row");

    assert!(
        store
            .is_participant(commission.id, seated.id)
            .await
            .expect("predicate"),
        "a membership row alone makes a Participant"
    );
}

// Ruling B2, the retroactive half — commissions created BEFORE the membership
// table existed get their owner's row backfilled by the migration (the ZMVP-71
// root pattern), stamped with the commission's creation instant.
#[tokio::test]
async fn the_migration_backfills_the_owners_participant_row() {
    let (pool, _container) = bare_pool().await;

    // Run every migration BEFORE the participant/seat one.
    let mut pre_membership = adapter_pg::migrator();
    let migrations: Vec<_> = pre_membership
        .migrations
        .iter()
        .filter(|m| m.version < PARTICIPANT_SEAT_MIGRATION)
        .cloned()
        .collect();
    assert!(
        !migrations.is_empty() && migrations.len() < pre_membership.migrations.len(),
        "the version constant matches an embedded migration"
    );
    pre_membership.migrations = migrations.into();
    pre_membership
        .run(&pool)
        .await
        .expect("pre-membership migrations run");

    // Seed a pre-membership world, exactly as earlier tickets wrote it.
    let owner = provision(&pool, "did:plc:early-adopter").await;
    let id = uuid::Uuid::now_v7();
    let created_at = Utc::now();
    sqlx::query(
        "INSERT INTO commission (id, title, owner_id, lifecycle, visibility, created_at)
         VALUES ($1, 'Pre-membership', $2, 'draft', 'private', $3)",
    )
    .bind(id)
    .bind(*owner.id)
    .bind(created_at)
    .execute(&pool)
    .await
    .expect("seed pre-membership commission");

    // The world catches up: the remaining migrations (with the backfill) run.
    adapter_pg::migrate(&pool).await.expect("catch-up migrates");

    let commission = CommissionId::new(id);
    assert_eq!(
        participant_rows(&pool, commission).await,
        vec![*owner.id],
        "the backfill seated the owner"
    );
    assert!(
        PgCommissionStore::new(pool.clone())
            .is_participant(commission, owner.id)
            .await
            .expect("predicate")
    );
}

// ZMVP-140 — a second add_participant for a pair that's already a member is a
// silent no-op (`ON CONFLICT (commission_id, user_id) DO NOTHING`): no
// duplicate row lands, and the original created_at is preserved rather than
// overwritten by the re-add's. This is the invariant ZMVP-79's seat
// acceptance will lean on — a User who already holds one seat and is seated
// into another must not double-insert or clobber their first membership
// instant.
#[tokio::test]
async fn add_participant_is_idempotent_for_an_already_seated_pair() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:reseat-owner").await;
    let seated = provision(&pool, "did:plc:reseat-seated").await;
    let commission = create_commission(&pool, &owner, "Reseated").await;

    let first_created_at = Utc::now();
    adapter_pg::queries::commission::add_participant(
        &pool,
        *commission.id,
        *seated.id,
        first_created_at,
    )
    .await
    .expect("first add lands a row");
    let stored_after_first = created_at_of(&pool, commission.id, seated.id).await;

    let second_created_at = first_created_at + chrono::Duration::hours(1);
    let rows_affected = adapter_pg::queries::commission::add_participant(
        &pool,
        *commission.id,
        *seated.id,
        second_created_at,
    )
    .await
    .expect("a re-add is a no-op, not an error");
    assert_eq!(rows_affected, 0, "the conflict is silently dropped");

    let rows_for_seated = participant_rows(&pool, commission.id)
        .await
        .into_iter()
        .filter(|id| *id == *seated.id)
        .count();
    assert_eq!(rows_for_seated, 1, "no duplicate row");

    let stored_after_second = created_at_of(&pool, commission.id, seated.id).await;
    assert_eq!(
        stored_after_second, stored_after_first,
        "the original created_at survives the re-add"
    );
}

/// The stored `created_at` for one participant membership row.
async fn created_at_of(
    pool: &PgPool,
    commission: CommissionId,
    user: UserId,
) -> chrono::DateTime<Utc> {
    sqlx::query_scalar(
        "SELECT created_at FROM commission_participant WHERE commission_id = $1 AND user_id = $2",
    )
    .bind(*commission)
    .bind(*user)
    .fetch_one(pool)
    .await
    .expect("fetch created_at")
}

// The permanent floor — the owner's participant row refuses a direct DELETE at
// the database (no port removes participants at all; this is the trigger's
// backstop for future code reaching past the ports), while a non-owner row
// (ZMVP-79's shape) stays freely removable.
#[tokio::test]
async fn the_owners_participant_row_is_irremovable_while_the_commission_lives() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:floor-owner").await;
    let seated = provision(&pool, "did:plc:floor-seated").await;
    let commission = create_commission(&pool, &owner, "Floored").await;

    sqlx::query(
        "INSERT INTO commission_participant (commission_id, user_id, created_at)
         VALUES ($1, $2, $3)",
    )
    .bind(*commission.id)
    .bind(*seated.id)
    .bind(Utc::now())
    .execute(&pool)
    .await
    .expect("seed a seated membership row");

    // Deleting the owner's row raises.
    let refused = sqlx::query("DELETE FROM commission_participant WHERE user_id = $1")
        .bind(*owner.id)
        .execute(&pool)
        .await;
    let err = refused.expect_err("the owner's row is the permanent floor");
    assert!(
        err.to_string().contains("permanent Participant"),
        "the trigger names the rule, got: {err}"
    );

    // Deleting the seated (non-owner) row is fine.
    sqlx::query("DELETE FROM commission_participant WHERE user_id = $1")
        .bind(*seated.id)
        .execute(&pool)
        .await
        .expect("a non-owner membership row deletes freely");

    assert_eq!(
        participant_rows(&pool, commission.id).await,
        vec![*owner.id],
        "the floor held; the seated row went"
    );
}

// Ruling E35 — the commission-delete cascade still sweeps EVERYTHING (what
// ZMVP-66's "gone entirely" relies on): participants (the floor trigger lets
// cascaded deletes through — the commission row is already gone when they
// fire), seats, and nodes all vanish with the commission row.
#[tokio::test]
async fn deleting_the_commission_cascades_participants_and_seats_away() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:cascade-owner").await;
    let commission = create_commission(&pool, &owner, "Doomed").await;
    let root = root_of(&pool, commission.id).await;

    let seat = NewSeat::under(
        commission.id,
        root,
        SeatKind::try_new("Creator").expect("valid kind"),
        None,
        None,
        owner.id,
        Utc::now(),
    );
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .declare_seat(&seat)
        .await
        .expect("declare seat");
    uow.commit().await.expect("commit");

    sqlx::query("DELETE FROM commission WHERE id = $1")
        .bind(*commission.id)
        .execute(&pool)
        .await
        .expect("the floor trigger must not block the commission's own deletion");

    let leftovers: (i64, i64, i64) = sqlx::query_as(
        "SELECT
            (SELECT count(*) FROM commission_participant WHERE commission_id = $1),
            (SELECT count(*) FROM commission_seat WHERE commission_id = $1),
            (SELECT count(*) FROM commission_node WHERE commission_id = $1)",
    )
    .bind(*commission.id)
    .fetch_one(&pool)
    .await
    .expect("count leftovers");
    assert_eq!(
        leftovers,
        (0, 0, 0),
        "participant/seat/node rows all cascade away with the commission"
    );
}

// AC1/AC2/AC3 (pg) — a declared seat lands as one component node plus its
// interpreted satellite sharing the id, atomically; seats() reads the kind,
// requirements, and the vacant occupant slot back; kinds repeat freely; and a
// rolled-back unit leaves neither half behind.
#[tokio::test]
async fn declare_seat_lands_a_node_and_its_satellite_together() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:declarer").await;
    let commission = create_commission(&pool, &owner, "Seatful").await;
    let root = root_of(&pool, commission.id).await;

    let first = NewSeat::under(
        commission.id,
        root,
        SeatKind::try_new("Creator").expect("valid kind"),
        Some(SeatPrompt::try_new("Two refs, please.").expect("valid prompt")),
        Some(SeatLink::try_new("https://forms.example/apply").expect("valid link")),
        owner.id,
        Utc::now(),
    );
    let second = NewSeat::under(
        commission.id,
        root,
        SeatKind::try_new("Creator").expect("valid kind"),
        None,
        None,
        owner.id,
        Utc::now(),
    );
    let (first_id, second_id) = (first.id, second.id);

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .declare_seat(&first)
        .await
        .expect("declare first");
    uow.commissions()
        .declare_seat(&second)
        .await
        .expect("declare second");
    uow.commit().await.expect("commit");

    let store = PgCommissionStore::new(pool.clone());
    let tree = store
        .load_tree(commission.id)
        .await
        .expect("load")
        .expect("tree exists");
    assert_eq!(tree.root.children.len(), 2);
    assert_eq!(tree.root.children[0].id, first_id, "append order");
    assert!(
        tree.root
            .children
            .iter()
            .all(|child| matches!(child.kind, NodeKind::Component)),
        "a seat's node is an ordinary component (the untyped v1 contract)"
    );

    let seats = store.seats(commission.id).await.expect("seats");
    assert_eq!(seats.len(), 2);
    let first_seat = seats.iter().find(|s| s.id == first_id).expect("first");
    assert_eq!(first_seat.kind.as_str(), "Creator");
    assert_eq!(
        first_seat.prompt.as_ref().map(|p| p.as_str()),
        Some("Two refs, please.")
    );
    assert_eq!(
        first_seat.link.as_ref().map(|l| l.as_str()),
        Some("https://forms.example/apply")
    );
    assert!(first_seat.is_vacant(), "born vacant (AC3)");
    let second_seat = seats.iter().find(|s| s.id == second_id).expect("second");
    assert_eq!(
        second_seat.kind.as_str(),
        "Creator",
        "kinds repeat freely (AC1)"
    );
    assert!(second_seat.prompt.is_none() && second_seat.link.is_none());

    // A rolled-back declaration leaves neither half behind.
    let third = NewSeat::under(
        commission.id,
        root,
        SeatKind::try_new("Client").expect("valid kind"),
        None,
        None,
        owner.id,
        Utc::now(),
    );
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .declare_seat(&third)
        .await
        .expect("declare third");
    uow.rollback().await.expect("rollback");
    assert_eq!(
        store.seats(commission.id).await.expect("seats").len(),
        2,
        "the rolled-back satellite never landed"
    );
    assert_eq!(
        store
            .load_tree(commission.id)
            .await
            .expect("load")
            .expect("tree exists")
            .root
            .children
            .len(),
        2,
        "the rolled-back node never landed"
    );
}

// The shared parent gate holds for seats too: a fabricated parent refuses with
// ParentNodeNotFound before anything lands.
#[tokio::test]
async fn declare_seat_refuses_an_absent_parent() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:gated").await;
    let commission = create_commission(&pool, &owner, "Gated").await;

    let fabricated = NewSeat::under(
        commission.id,
        NodeId::new(uuid::Uuid::now_v7()),
        SeatKind::try_new("Creator").expect("valid kind"),
        None,
        None,
        owner.id,
        Utc::now(),
    );
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let err = uow
        .commissions()
        .declare_seat(&fabricated)
        .await
        .expect_err("absent parent refuses");
    assert!(
        err.downcast_ref::<ParentNodeNotFound>().is_some(),
        "expected ParentNodeNotFound, got: {err:?}"
    );
    uow.rollback().await.expect("rollback");

    assert!(
        PgCommissionStore::new(pool.clone())
            .seats(commission.id)
            .await
            .expect("seats")
            .is_empty()
    );
}
