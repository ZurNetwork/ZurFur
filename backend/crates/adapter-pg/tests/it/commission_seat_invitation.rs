//! Seat invitations over PostgreSQL (ZMVP-78), against a throwaway container:
//! creating a pending invitation persists a row `find_pending_seat_invitation`
//! reads back; the partial unique index bars a second pending offer for the same
//! (seat, user) pair while letting several *different* Users hold pending offers
//! to one Seat; and a revoke flips the row to revoked, clearing the pending
//! offer. The Seat mirror of the account-invitation suite. Requires a container
//! runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            Commission, CommissionId, CommissionTitle, NewSeat, NodeId, SeatInvitation, SeatKind,
        },
        did::Did,
        invitation::InvitationState,
        user::{User, UserId},
    },
    ports::{CommissionStore, Database},
};

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container alive
/// for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// Recognize a visitor in its own committed unit of work
/// (`commission_invitation.invited_user`/`inviter` reference `users(id)`).
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

/// Create a commission (which mints its root and its owner's participant row) in
/// one committed unit of work.
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

/// Declare a vacant Seat under `parent` in one committed unit of work; returns
/// its node id (the seat invitations reference it).
async fn declare_seat(
    pool: &PgPool,
    commission: CommissionId,
    parent: NodeId,
    owner: &User,
) -> NodeId {
    let seat = NewSeat::under(
        commission,
        parent,
        SeatKind::try_new("Creator").expect("valid kind"),
        None,
        None,
        owner.id,
        Utc::now(),
    );
    let seat_id = seat.id;
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .declare_seat(&seat)
        .await
        .expect("declare seat");
    uow.commit().await.expect("commit");
    seat_id
}

/// Issue a pending seat invitation in one committed unit of work.
async fn issue(pool: &PgPool, invitation: &SeatInvitation) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .create_seat_invitation(invitation)
        .await
        .expect("create seat invitation");
    uow.commit().await.expect("commit");
}

/// How many `commission_invitation` rows exist for `(seat, user)` in any state —
/// the raw row count, so a "no second row" claim is proven against the table.
async fn rows_for(pool: &PgPool, seat: NodeId, user: UserId) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM commission_invitation WHERE seat_id = $1 AND invited_user = $2",
    )
    .bind(*seat)
    .bind(*user)
    .fetch_one(pool)
    .await
    .expect("count commission_invitation")
}

// A created pending invitation is read back by find_pending_seat_invitation with
// all its facts intact.
#[tokio::test]
async fn create_then_find_pending_returns_the_invitation() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:seat-inv-owner").await;
    let invitee = provision(&pool, "did:plc:seat-inv-invitee").await;
    let commission = create_commission(&pool, &owner, "Invited").await;
    let root = root_of(&pool, commission.id).await;
    let seat = declare_seat(&pool, commission.id, root, &owner).await;

    let invitation = SeatInvitation::issue(commission.id, seat, invitee.id, owner.id, Utc::now());
    let invitation_id = invitation.id;
    issue(&pool, &invitation).await;

    let store = PgCommissionStore::new(pool.clone());
    let found = store
        .find_pending_seat_invitation(commission.id, seat, invitee.id)
        .await
        .expect("query")
        .expect("the pending offer is found");
    assert_eq!(found.id, invitation_id);
    assert_eq!(found.commission, commission.id);
    assert_eq!(found.seat, seat);
    assert_eq!(found.invited_user, invitee.id);
    assert_eq!(found.inviter, owner.id);
    assert_eq!(found.state, InvitationState::Pending);

    // A user with no offer to this seat finds nothing.
    let stranger = provision(&pool, "did:plc:seat-inv-stranger").await;
    assert!(
        store
            .find_pending_seat_invitation(commission.id, seat, stranger.id)
            .await
            .expect("query")
            .is_none(),
        "an uninvited user has no pending offer"
    );
}

// The partial unique index bars a *second* pending offer for the same
// (seat, user): a re-issue is a no-op (ON CONFLICT DO NOTHING), never a second row.
#[tokio::test]
async fn a_second_pending_invitation_for_the_same_pair_is_not_a_second_row() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:dup-owner").await;
    let invitee = provision(&pool, "did:plc:dup-invitee").await;
    let commission = create_commission(&pool, &owner, "Duped").await;
    let root = root_of(&pool, commission.id).await;
    let seat = declare_seat(&pool, commission.id, root, &owner).await;

    let first = SeatInvitation::issue(commission.id, seat, invitee.id, owner.id, Utc::now());
    let first_id = first.id;
    issue(&pool, &first).await;
    // A fresh SeatInvitation (distinct id) for the same pair — the store drops it.
    let second = SeatInvitation::issue(commission.id, seat, invitee.id, owner.id, Utc::now());
    assert_ne!(first_id, second.id, "a distinct offer object");
    issue(&pool, &second).await;

    assert_eq!(
        rows_for(&pool, seat, invitee.id).await,
        1,
        "the duplicate pending issue is a no-op, not a second row"
    );
    let found = PgCommissionStore::new(pool.clone())
        .find_pending_seat_invitation(commission.id, seat, invitee.id)
        .await
        .expect("query")
        .expect("the original offer stands");
    assert_eq!(found.id, first_id, "the first offer is the one kept");
}

// Several *different* Users may hold pending invitations to ONE Seat at once —
// the acceptance race is ZMVP-79's to resolve, not this table's to forbid.
#[tokio::test]
async fn two_users_may_hold_pending_invitations_to_one_seat() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:race-owner").await;
    let alice = provision(&pool, "did:plc:race-alice").await;
    let bob = provision(&pool, "did:plc:race-bob").await;
    let commission = create_commission(&pool, &owner, "Contested").await;
    let root = root_of(&pool, commission.id).await;
    let seat = declare_seat(&pool, commission.id, root, &owner).await;

    for invitee in [&alice, &bob] {
        issue(
            &pool,
            &SeatInvitation::issue(commission.id, seat, invitee.id, owner.id, Utc::now()),
        )
        .await;
    }

    let store = PgCommissionStore::new(pool.clone());
    assert!(
        store
            .find_pending_seat_invitation(commission.id, seat, alice.id)
            .await
            .expect("query")
            .is_some(),
        "Alice holds a pending offer to the seat"
    );
    assert!(
        store
            .find_pending_seat_invitation(commission.id, seat, bob.id)
            .await
            .expect("query")
            .is_some(),
        "Bob holds one too — two users, one seat"
    );
    let seat_total: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM commission_invitation WHERE seat_id = $1 AND state = 'pending'",
    )
    .bind(*seat)
    .fetch_one(&pool)
    .await
    .expect("count");
    assert_eq!(seat_total, 2, "both pending offers coexist on the seat");
}

// Revoke flips the row to revoked and clears the pending offer; a second revoke
// is a harmless no-op.
#[tokio::test]
async fn revoke_flips_state_and_clears_the_pending_offer() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:rev-owner").await;
    let invitee = provision(&pool, "did:plc:rev-invitee").await;
    let commission = create_commission(&pool, &owner, "Revoked").await;
    let root = root_of(&pool, commission.id).await;
    let seat = declare_seat(&pool, commission.id, root, &owner).await;

    let invitation = SeatInvitation::issue(commission.id, seat, invitee.id, owner.id, Utc::now());
    let invitation_id = invitation.id;
    issue(&pool, &invitation).await;

    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .revoke_seat_invitation(invitation_id)
        .await
        .expect("revoke");
    uow.commit().await.expect("commit");

    let store = PgCommissionStore::new(pool.clone());
    assert!(
        store
            .find_pending_seat_invitation(commission.id, seat, invitee.id)
            .await
            .expect("query")
            .is_none(),
        "a revoked offer is no longer a pending offer"
    );
    // The row itself survives as revoked history (the partial index only spans pending).
    let state: String = sqlx::query_scalar("SELECT state FROM commission_invitation WHERE id = $1")
        .bind(*invitation_id)
        .fetch_one(&pool)
        .await
        .expect("row still present");
    assert_eq!(state, "revoked", "the row is revoked history, not deleted");

    // A second revoke is an idempotent no-op — nothing to flip.
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .revoke_seat_invitation(invitation_id)
        .await
        .expect("second revoke is a no-op");
    uow.commit().await.expect("commit");
}

// The pending lookup is scoped to its commission IN THE QUERY: another
// commission's id never reaches this commission's offer, even holding the real
// seat id — the authorization binding is unrepresentable to skip (the revoke
// handler authorizes against the path commission and passes it here, so a
// cross-commission seat id resolves to nothing and revoke is a no-op).
#[tokio::test]
async fn find_pending_is_scoped_to_its_commission() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:scope-owner").await;
    let invitee = provision(&pool, "did:plc:scope-invitee").await;
    let commission = create_commission(&pool, &owner, "Scoped").await;
    let root = root_of(&pool, commission.id).await;
    let seat = declare_seat(&pool, commission.id, root, &owner).await;
    let other = create_commission(&pool, &owner, "Other").await;

    let invitation = SeatInvitation::issue(commission.id, seat, invitee.id, owner.id, Utc::now());
    issue(&pool, &invitation).await;

    let store = PgCommissionStore::new(pool.clone());
    assert!(
        store
            .find_pending_seat_invitation(other.id, seat, invitee.id)
            .await
            .expect("query")
            .is_none(),
        "another commission's id never reaches this commission's offer"
    );
    assert!(
        store
            .find_pending_seat_invitation(commission.id, seat, invitee.id)
            .await
            .expect("query")
            .is_some(),
        "the owning commission still finds it"
    );
}
