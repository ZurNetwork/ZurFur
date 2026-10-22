#![allow(
    clippy::disallowed_methods,
    reason = "a test opens its unit by hand and reads the node rows back"
)]

mod spy;

use std::sync::Arc;

use adapter_mem::fixture::{alice, bob, old_sketch, untitled};
use atta::{WriteCaps, WriteError, Writer};
use chrono::Utc;
use domain::{
    elements::{commission::ChangelogEntry, did::Did, level::Level, user::UserId},
    ports::{Database, UnitOfWork},
};
use test_support::pg_app::{StatementLog, pg_runtime, pg_runtime_recording};

use super::*;
use crate::{
    Ports,
    den::{
        Den, DenThing,
        authority::{GrantKind, MINT_AS},
        probe::{
            Probe, ProbeHandler, commission_path, create_act, hand_built_create, hand_built_rename,
        },
        write::{Command, DenWriteError, WriteResult},
    },
    test_world, transaction,
};
use spy::{Spy, SpyDatabase};

/// The title every create here gives its commission, which no entry may carry.
const TITLE: &str = "A gated piece";

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

/// `actor`'s write of `act` at `now`, through the Den's write.
async fn write_at(
    ports: &Ports,
    actor: &UserId,
    act: DenAct,
    now: DateTimeUtc,
) -> Result<WriteResult, DenWriteError> {
    let command = Command {
        actor: actor.clone(),
        act,
    };
    Den::from(ports).write(command, now).await
}

/// The commission a write made.
fn made(written: &WriteResult) -> CommissionId {
    let DenThing::Commission(commission) = written.thing else {
        panic!("a create names a commission: {written:?}");
    };
    commission
}

/// The commission `commission` as the store holds it, if it does.
async fn stored(ports: &Ports, commission: CommissionId) -> Option<Commission> {
    ports
        .commissions
        .find(&commission)
        .await
        .expect("the store reads")
}

/// Every changelog entry of `commission`.
async fn entries(ports: &Ports, commission: CommissionId) -> Vec<ChangelogEntry> {
    ports
        .changelog
        .entries(&commission)
        .await
        .expect("the changelog reads")
}

/// The user `did`, recognized.
async fn provision(database: &dyn Database, did: &str) -> UserId {
    let did = Did::from(did.to_owned());
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let user = uow.users().provision(&did).await?;
        Ok(user.id)
    })
    .await
    .expect("the user is recognized")
}

/// The fixture world, its units spied on by `spy`.
async fn spied(spy: &Spy) -> test_world::World {
    let mut world = test_world::world().await;
    let database = SpyDatabase::new(world.ports.database.clone(), spy.clone());
    world.ports.database = Arc::new(database);
    world
}

/// Each creator that drops a failure it got through its view, then answers as if none came.
const SWALLOWING: [ProbeHandler; 2] =
    [ProbeHandler::SwallowMiss, ProbeHandler::SwallowStoreFailure];

/// Each swallowing creator's act on `ports` by `actor` fails as a store failure, and keeps
/// neither its node nor its commission.
async fn each_swallowed_failure_fails_the_act(ports: &Ports, actor: &UserId) {
    for handler in SWALLOWING {
        let planted = CommissionId::mint();
        let act = Probe::create(TITLE).handled(handler.clone()).act();

        let written = MINT_AS
            .scope(planted, write_at(ports, actor, act, Utc::now()))
            .await;

        assert!(
            matches!(written, Err(DenWriteError::Store(_))),
            "{handler:?}: {written:?}"
        );
        assert!(
            top_row(&*ports.database, planted).await.is_none(),
            "{handler:?}"
        );
        assert!(stored(ports, planted).await.is_none(), "{handler:?}");
    }
}

#[tokio::test]
async fn a_swallowed_store_failure_under_a_handler_fails_the_act() {
    let world = test_world::world().await;

    each_swallowed_failure_fails_the_act(&world.ports, &alice()).await;
}

#[tokio::test]
async fn a_swallowed_store_failure_under_a_handler_fails_the_act_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let actor = provision(&*ports.database, "did:plc:gate-swallowing").await;

    each_swallowed_failure_fails_the_act(&ports, &actor).await;
}

// A repo whose read answers another commission's row: the handler answers what it read, and
// the gate refuses an answer that isn't the grant's commission.
#[tokio::test]
async fn an_answer_naming_another_commission_fails_the_act() {
    let spy = Spy::answering_as(old_sketch());
    let world = spied(&spy).await;
    let planted = CommissionId::mint();

    let written = MINT_AS
        .scope(
            planted,
            write_at(&world.ports, &alice(), create_act(TITLE), Utc::now()),
        )
        .await;

    assert!(
        matches!(written, Err(DenWriteError::Store(_))),
        "{written:?}"
    );
    assert!(top_row(&*world.ports.database, planted).await.is_none());
    assert!(stored(&world.ports, planted).await.is_none());
}

#[tokio::test]
async fn the_created_commission_points_at_the_node_the_gate_made() {
    let spy = Spy::default();
    let world = spied(&spy).await;

    let written = write_at(&world.ports, &alice(), create_act(TITLE), Utc::now())
        .await
        .expect("alice creates");

    let node = top_row(&*world.ports.database, made(&written))
        .await
        .expect("its node is written");
    let bound: Vec<NodeId> = spy
        .recorded_births()
        .iter()
        .map(|birth| birth.node)
        .collect();
    assert_eq!(bound, [node.id]);
}

#[tokio::test]
async fn the_created_commission_is_owned_by_the_grant_s_actor() {
    let spy = Spy::default();
    let world = spied(&spy).await;
    let now = Utc::now();

    let written = write_at(&world.ports, &bob(), create_act(TITLE), now)
        .await
        .expect("bob creates");

    let births = spy.recorded_births();
    let owners: Vec<(UserId, DateTimeUtc)> = births
        .iter()
        .map(|birth| (birth.owner_id.clone(), birth.created_at))
        .collect();
    assert_eq!(owners, [(bob(), now)]);
    let commission = stored(&world.ports, made(&written))
        .await
        .expect("the commission is stored");
    assert!(commission.is_owned_by(&bob()));
    assert_eq!(commission.created_at, now);
}

/// A create by `actor` on `ports` writes exactly one entry, `created`.
async fn one_created_entry(ports: &Ports, actor: &UserId) {
    let written = write_at(ports, actor, create_act(TITLE), Utc::now())
        .await
        .expect("the create lands");

    let kinds: Vec<ChangelogEntryKind> = entries(ports, made(&written))
        .await
        .iter()
        .map(|entry| entry.kind)
        .collect();
    assert_eq!(kinds, [ChangelogEntryKind::Created]);
}

#[tokio::test]
async fn a_create_writes_exactly_one_created_entry() {
    let world = test_world::world().await;

    one_created_entry(&world.ports, &alice()).await;
}

#[tokio::test]
async fn a_create_writes_exactly_one_created_entry_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let actor = provision(&*ports.database, "did:plc:gate-one-entry").await;

    one_created_entry(&ports, &actor).await;
}

/// A create by `actor` on `ports` writes its entry as the gate builds it: on the made
/// commission, by `actor`, with an empty payload, no note and no title anywhere.
async fn the_entry_is_the_gate_s(ports: &Ports, actor: &UserId) {
    let written = write_at(ports, actor, create_act(TITLE), Utc::now())
        .await
        .expect("the create lands");

    let minted = made(&written);
    let stream = entries(ports, minted).await;
    let [entry] = stream.as_slice() else {
        panic!("one entry: {} of them", stream.len());
    };
    let empty_payload = serde_json::json!({});
    assert_eq!(entry.commission_id, minted);
    assert_eq!(entry.actor_id.as_ref(), Some(actor));
    assert_eq!(entry.payload, empty_payload);
    assert_eq!(entry.note, None);
    assert!(!entry.payload.to_string().contains(TITLE));
}

#[tokio::test]
async fn the_gate_s_entries_carry_an_empty_payload_and_no_note() {
    let world = test_world::world().await;

    the_entry_is_the_gate_s(&world.ports, &alice()).await;
}

#[tokio::test]
async fn the_gate_s_entries_carry_an_empty_payload_and_no_note_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let actor = provision(&*ports.database, "did:plc:gate-entry").await;

    the_entry_is_the_gate_s(&ports, &actor).await;
}

// The entry's append fails after the node rows and the commission are written: the unit
// rolls back whole, so neither is kept.
#[tokio::test]
async fn a_failed_create_leaves_no_node() {
    let spy = Spy::refusing_appends();
    let world = spied(&spy).await;
    let planted = CommissionId::mint();

    let written = MINT_AS
        .scope(
            planted,
            write_at(&world.ports, &alice(), create_act(TITLE), Utc::now()),
        )
        .await;

    assert!(
        matches!(written, Err(DenWriteError::Store(_))),
        "{written:?}"
    );
    assert_eq!(spy.recorded_births().len(), 1, "the create ran first");
    assert!(top_row(&*world.ports.database, planted).await.is_none());
    assert!(stored(&world.ports, planted).await.is_none());
}

// On PostgreSQL, a create writes its seven node rows and reads none: its answer comes from
// what the gate wrote and the handler's commission read.
#[tokio::test]
async fn create_reads_no_node_after_writing() {
    let statements = StatementLog::default();
    let pg = pg_runtime_recording(&statements).await;
    let ports = Ports::from(&pg.runtime);
    let actor = provision(&*ports.database, "did:plc:gate-no-node-read").await;
    let mark = statements.mark();

    write_at(&ports, &actor, create_act(TITLE), Utc::now())
        .await
        .expect("the create lands");

    let node_statements: Vec<&str> = statements
        .since(mark)
        .iter()
        .filter_map(|statement| statement.query_name())
        .filter(|name| name.starts_with("node/"))
        .collect();
    assert_eq!(node_statements, ["node/create"; 7]);
}
