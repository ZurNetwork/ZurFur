#![allow(
    clippy::disallowed_methods,
    reason = "a test opens its unit by hand and reads the node rows back"
)]

use adapter_mem::fixture::{alice, bob, untitled};
use atta::{WriteCaps, WriteError, Writer};
use chrono::Utc;
use domain::{
    elements::{level::Level, node::NodeSegment, user::UserId},
    ports::{Database, UnitOfWork},
};

use super::*;
use crate::{
    den::{
        authority::GrantKind,
        probe::{Probe, commission_path, hand_built_create, hand_built_rename},
    },
    test_world,
};

/// A grant on "Untitled" to `actor` in `role`.
fn existing(actor: &UserId, role: CommissionRole) -> Grant {
    let kind = GrantKind::Existing {
        commission: untitled(),
        role,
        level: Level::Private,
    };
    Grant::for_test(actor, kind)
}

/// A create grant to `actor` under `commission`.
fn creating(actor: &UserId, commission: CommissionId) -> Grant {
    let kind = GrantKind::Create {
        commission: New::from(commission),
    };
    Grant::for_test(actor, kind)
}

/// A commission id nothing is stored under.
fn made_up() -> CommissionId {
    CommissionId::from(uuid::Uuid::now_v7())
}

/// The top-level node row of `commission`, read on a unit that keeps nothing.
async fn top_row(database: &dyn Database, commission: CommissionId) -> Option<NodeRow> {
    let mut unit = database.begin().await.expect("begin");
    let segment = NodeSegment::try_from(commission.to_string()).expect("an id is a segment");
    let row = unit
        .nodes()
        .at_top(&NodeType::Commission, &segment)
        .await
        .expect("the node row reads");
    unit.rollback().await.expect("the read's unit ends");
    row
}

/// The commission node for "Untitled", named "Untitled".
async fn seed_untitled(database: &dyn Database) {
    let node = NewNode {
        id: NodeId::mint(),
        parent: None,
        segment: NodeSegment::try_from(untitled().to_string()).expect("a segment"),
        node_type: NodeType::Commission,
        level: Level::Private,
        name: StoredText::from("Untitled".to_owned()),
    };
    crate::transaction(database, async move |uow: &mut dyn UnitOfWork| {
        uow.nodes().create(&node).await
    })
    .await
    .expect("the node seeds");
}

/// `act` through a write host for `grant`, as `viewer`, on a unit committed only when the gate
/// applied it.
async fn through_host(
    database: &dyn Database,
    viewer: Viewer,
    grant: Grant,
    act: atta::Act<DenAct>,
) -> Result<Result<DenApplied, DenRefusal>, WriteError> {
    let mut unit = database.begin().await.expect("begin");
    let writer = Writer {
        viewer,
        caps: WriteCaps::default(),
    };
    let mut host = DenWriteHost::new(&mut *unit, grant, Utc::now());
    let outcome = writer.act(&mut host, act).await;
    drop(host);
    if matches!(outcome, Ok(Ok(_))) {
        unit.commit().await.expect("commit");
    } else {
        unit.rollback().await.expect("rollback");
    }
    outcome
}

#[test]
fn an_owner_capability_is_never_issued_to_a_participant() {
    let participant = existing(&bob(), CommissionRole::Participant);
    let owner = existing(&alice(), CommissionRole::Owner);
    let create = creating(&alice(), made_up());

    assert!(issue::<AsOwner>(&participant).is_none());
    assert!(issue::<AsParticipant>(&participant).is_some());
    assert!(issue::<AsOwner>(&owner).is_some());
    assert!(issue::<AsParticipant>(&owner).is_some());
    assert!(issue::<AsOwner>(&create).is_none());
    assert!(issue::<AsParticipant>(&create).is_none());
    assert!(issue::<Creator>(&create).is_some());
    assert!(issue::<Creator>(&owner).is_none());
}

#[tokio::test]
async fn the_write_host_refuses_another_viewer() {
    let world = test_world::world().await;
    let database = &*world.ports.database;
    seed_untitled(database).await;
    let probe = Probe::rename(untitled(), "Taken over").needing(CommissionRole::Owner);
    let act = hand_built_rename(commission_path(untitled()), "Taken over", probe);

    let applied = through_host(
        database,
        DenHost::viewer_of(&bob()),
        existing(&alice(), CommissionRole::Owner),
        act,
    )
    .await;

    assert!(matches!(applied, Err(WriteError::Host(_))), "{applied:?}");
    let row = top_row(database, untitled()).await.expect("seeded");
    assert_eq!(row.name.as_str(), "Untitled");
}

#[tokio::test]
async fn a_create_under_a_directory_passes_through_the_write_host() {
    let world = test_world::world().await;
    let database = &*world.ports.database;
    let minted = made_up();
    let act = hand_built_create(minted, "Made here", Probe::create("Made here"));

    let applied = through_host(
        database,
        DenHost::viewer_of(&alice()),
        creating(&alice(), minted),
        act,
    )
    .await
    .expect("Atta passes a create under a directory")
    .expect("the gate writes it");

    assert_eq!(applied.commission, minted);
    assert_eq!(applied.level, atta::Level::Private);
    let row = top_row(database, minted)
        .await
        .expect("its node is written");
    assert_eq!(row.name.as_str(), "Made here");
}
