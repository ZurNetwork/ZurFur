//! The shared commission-standing conformance suite, run by both adapters: `adapter-mem`'s
//! `tests/commission_standing.rs` and `adapter-pg`'s `tests/it/commission_standing.rs`.
//!
//! Each case takes a fresh [`Database`], the [`CommissionStore`] over the same store, the
//! adapter's [`ParticipantRows`] and its [`CommissionSeeder`], and checks the pool's `standing`
//! and a unit's `standing_for_update` alike wherever both apply, and the node the locked read
//! answers.
//! [`commission_standing_contract_tests!`](crate::commission_standing_contract_tests) writes one
//! test per case.

use async_trait::async_trait;
use chrono::Utc;
use domain::elements::{
    commission::{CommissionId, CommissionRole, CommissionStanding, LockedStanding},
    did::Did,
    level::Level,
    node::{NodeId, NodeSegment, NodeType},
    user::UserId,
};
use domain::ports::{CommissionStore, Database};
use strum::VariantArray;

use crate::seeder::{CommissionSeeder, seed_commission, write_commission};

/// The name every commission node a case seeds holds.
const MADE_UP_NAME: &str = "A made-up commission";

/// The participant rows a case needs, written each adapter's own way. A trait of its own:
/// no port adds a non-owner participant row or removes one today.
#[async_trait]
pub trait ParticipantRows: Send + Sync {
    /// Adds `user`'s participant row on `commission`, committed.
    async fn add(&self, commission: &CommissionId, user: &UserId);

    /// Removes `user`'s participant row on `commission`, committed, the owner's included.
    async fn remove(&self, commission: &CommissionId, user: &UserId);
}

/// Writes one `#[tokio::test]` per suite case. `$setup` is an async fn returning
/// `(Arc<dyn Database>, Arc<dyn CommissionStore>, Arc<dyn ParticipantRows>,
/// Arc<dyn CommissionSeeder>, impl Sized)` over one fresh store; the fifth item is kept alive
/// for the test.
#[macro_export]
macro_rules! commission_standing_contract_tests {
    ($setup:path) => {
        $crate::commission_standing_contract_tests!(@cases $setup;
            the_owner_is_owner_without_a_participant_row,
            a_participant_is_a_participant,
            a_stranger_has_no_role,
            an_absent_commission_is_not_found,
            archived_is_reported,
            the_level_is_reported,
            the_locked_read_sees_the_unit_s_own_writes,
            the_locked_read_answers_the_commission_s_node,
        );
    };
    (@cases $setup:path; $($case:ident),* $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                let (database, store, rows, seeder, _kept) = $setup().await;
                $crate::commission_standing_contract::$case(
                    &*database, &*store, &*rows, &*seeder,
                )
                .await;
            }
        )*
    };
}

/// Recognizes `did` as a User in a unit of its own.
async fn provision(database: &dyn Database, did: &str) -> UserId {
    let mut unit = database.begin().await.expect("begin");
    let user = unit
        .users()
        .provision(&Did::from(did.to_owned()))
        .await
        .expect("provision the user");
    unit.commit().await.expect("commit the user");
    user.id
}

/// Archives `commission` in a unit of its own.
async fn archive(database: &dyn Database, commission: &CommissionId) {
    let mut unit = database.begin().await.expect("begin");
    let archived = unit
        .commissions()
        .set_archived(commission, Some(Utc::now()))
        .await
        .expect("archive the commission");
    assert!(archived, "the commission was live");
    unit.commit().await.expect("commit the archive");
}

/// `user`'s standing on `commission` through a unit's `standing_for_update`, beside the node
/// it answered; the unit is rolled back after.
async fn locked_standing(
    database: &dyn Database,
    commission: &CommissionId,
    user: &UserId,
) -> Option<LockedStanding> {
    let mut unit = database.begin().await.expect("begin");
    let standing = unit
        .commissions()
        .standing_for_update(commission, user)
        .await
        .expect("the locked standing read");
    unit.rollback().await.expect("roll the read back");
    standing
}

/// The node `commission` stands for at the top of the tree, read on a unit that keeps nothing.
async fn top_node(database: &dyn Database, commission: &CommissionId) -> Option<NodeId> {
    let segment = NodeSegment::try_from(commission.to_string()).expect("an id is a segment");
    let mut unit = database.begin().await.expect("begin");
    let row = unit
        .nodes()
        .at_top(&NodeType::Commission, &segment)
        .await
        .expect("the node row reads");
    unit.rollback().await.expect("roll the read back");
    row.map(|row| row.id)
}

/// Asserts the pool's `standing` and a unit's `standing_for_update` both answer `expected`.
async fn assert_both_read(
    database: &dyn Database,
    store: &dyn CommissionStore,
    commission: &CommissionId,
    user: &UserId,
    expected: Option<CommissionStanding>,
) {
    let pooled = store
        .standing(commission, user)
        .await
        .expect("the pool's standing read");
    assert_eq!(pooled, expected, "standing");
    let locked = locked_standing(database, commission, user).await;
    let locked_standing = locked.map(|locked| locked.standing);
    assert_eq!(locked_standing, expected, "standing_for_update");
}

/// A live Private standing holding `role`.
fn private_standing(role: Option<CommissionRole>) -> CommissionStanding {
    CommissionStanding {
        role,
        level: Level::Private,
        archived: false,
    }
}

/// The owner is Owner on the commission row alone, with no participant row behind it.
pub async fn the_owner_is_owner_without_a_participant_row(
    database: &dyn Database,
    store: &dyn CommissionStore,
    rows: &dyn ParticipantRows,
    seeder: &dyn CommissionSeeder,
) {
    let owner = provision(database, "did:plc:standingowner").await;
    let commission = seed_commission(database, seeder, &owner, Level::Private, MADE_UP_NAME).await;
    rows.remove(&commission, &owner).await;
    let still_a_participant = store
        .is_participant(&commission, &owner)
        .await
        .expect("is_participant");
    assert!(!still_a_participant, "the owner's row is gone");

    let expected = private_standing(Some(CommissionRole::Owner));
    assert_both_read(database, store, &commission, &owner, Some(expected)).await;
}

/// A user with a participant row who doesn't own the commission is a Participant.
pub async fn a_participant_is_a_participant(
    database: &dyn Database,
    store: &dyn CommissionStore,
    rows: &dyn ParticipantRows,
    seeder: &dyn CommissionSeeder,
) {
    let owner = provision(database, "did:plc:standingowner").await;
    let member = provision(database, "did:plc:standingmember").await;
    let commission = seed_commission(database, seeder, &owner, Level::Private, MADE_UP_NAME).await;
    rows.add(&commission, &member).await;

    let expected = private_standing(Some(CommissionRole::Participant));
    assert_both_read(database, store, &commission, &member, Some(expected)).await;
}

/// A user who neither owns the commission nor holds a row finds it, with no role.
pub async fn a_stranger_has_no_role(
    database: &dyn Database,
    store: &dyn CommissionStore,
    _rows: &dyn ParticipantRows,
    seeder: &dyn CommissionSeeder,
) {
    let owner = provision(database, "did:plc:standingowner").await;
    let stranger = provision(database, "did:plc:standingstranger").await;
    let commission = seed_commission(database, seeder, &owner, Level::Private, MADE_UP_NAME).await;

    let expected = private_standing(None);
    assert_both_read(database, store, &commission, &stranger, Some(expected)).await;
}

/// An id no commission holds answers not found, on both reads.
pub async fn an_absent_commission_is_not_found(
    database: &dyn Database,
    store: &dyn CommissionStore,
    _rows: &dyn ParticipantRows,
    seeder: &dyn CommissionSeeder,
) {
    let owner = provision(database, "did:plc:standingowner").await;
    seed_commission(database, seeder, &owner, Level::Private, MADE_UP_NAME).await;
    let absent = CommissionId::from(uuid::Uuid::now_v7());

    assert_both_read(database, store, &absent, &owner, None).await;
}

/// An archived commission is found, and reported archived.
pub async fn archived_is_reported(
    database: &dyn Database,
    store: &dyn CommissionStore,
    _rows: &dyn ParticipantRows,
    seeder: &dyn CommissionSeeder,
) {
    let owner = provision(database, "did:plc:standingowner").await;
    let commission = seed_commission(database, seeder, &owner, Level::Private, MADE_UP_NAME).await;
    archive(database, &commission).await;

    let expected = CommissionStanding {
        role: Some(CommissionRole::Owner),
        level: Level::Private,
        archived: true,
    };
    assert_both_read(database, store, &commission, &owner, Some(expected)).await;
}

/// Each level a commission is stored at is the level its standing reports.
pub async fn the_level_is_reported(
    database: &dyn Database,
    store: &dyn CommissionStore,
    _rows: &dyn ParticipantRows,
    seeder: &dyn CommissionSeeder,
) {
    let owner = provision(database, "did:plc:standingowner").await;
    for level in Level::VARIANTS {
        let commission = seed_commission(database, seeder, &owner, *level, MADE_UP_NAME).await;
        let expected = CommissionStanding {
            role: Some(CommissionRole::Owner),
            level: *level,
            archived: false,
        };
        assert_both_read(database, store, &commission, &owner, Some(expected)).await;
    }
}

/// Inside one unit, a commission created and archived there is seen by `standing_for_update`
/// on that unit, while the pool can't see it yet.
pub async fn the_locked_read_sees_the_unit_s_own_writes(
    database: &dyn Database,
    store: &dyn CommissionStore,
    _rows: &dyn ParticipantRows,
    seeder: &dyn CommissionSeeder,
) {
    let owner = provision(database, "did:plc:standingowner").await;
    let stranger = provision(database, "did:plc:standingstranger").await;
    let mut unit = database.begin().await.expect("begin");
    let commission =
        write_commission(&mut *unit, seeder, &owner, Level::Listed, MADE_UP_NAME).await;

    let created = unit
        .commissions()
        .standing_for_update(&commission, &owner)
        .await
        .expect("the locked standing read");
    let owner_standing = CommissionStanding {
        role: Some(CommissionRole::Owner),
        level: Level::Listed,
        archived: false,
    };
    let created = created.map(|locked| locked.standing);
    assert_eq!(created, Some(owner_standing), "the owner, after the create");
    let seen_by_a_stranger = unit
        .commissions()
        .standing_for_update(&commission, &stranger)
        .await
        .expect("the locked standing read");
    let stranger_standing = CommissionStanding {
        role: None,
        ..owner_standing
    };
    let seen_by_a_stranger = seen_by_a_stranger.map(|locked| locked.standing);
    assert_eq!(
        seen_by_a_stranger,
        Some(stranger_standing),
        "a stranger, after the create"
    );

    unit.commissions()
        .set_archived(&commission, Some(Utc::now()))
        .await
        .expect("archive the commission");
    let archived = unit
        .commissions()
        .standing_for_update(&commission, &owner)
        .await
        .expect("the locked standing read");
    let archived_standing = CommissionStanding {
        archived: true,
        ..owner_standing
    };
    let archived = archived.map(|locked| locked.standing);
    assert_eq!(archived, Some(archived_standing), "after the archive");

    let pooled = store
        .standing(&commission, &owner)
        .await
        .expect("the pool's standing read");
    assert_eq!(
        pooled, None,
        "the pool can't see the unit's uncommitted create"
    );
    unit.rollback().await.expect("roll the unit back");
}

/// The locked read answers the node the commission points at, for its owner and a stranger
/// alike: the commission's own node at the top of the tree.
pub async fn the_locked_read_answers_the_commission_s_node(
    database: &dyn Database,
    _store: &dyn CommissionStore,
    _rows: &dyn ParticipantRows,
    seeder: &dyn CommissionSeeder,
) {
    let owner = provision(database, "did:plc:standingowner").await;
    let stranger = provision(database, "did:plc:standingstranger").await;
    let commission = seed_commission(database, seeder, &owner, Level::Private, MADE_UP_NAME).await;
    seed_commission(database, seeder, &owner, Level::Private, MADE_UP_NAME).await;

    let node = top_node(database, &commission).await;
    let owner_s = locked_standing(database, &commission, &owner).await;
    let stranger_s = locked_standing(database, &commission, &stranger).await;

    assert!(node.is_some(), "the seeded commission has its node");
    assert_eq!(owner_s.map(|locked| locked.node), node, "the owner's read");
    assert_eq!(
        stranger_s.map(|locked| locked.node),
        node,
        "a stranger's read"
    );
}
