#![allow(
    clippy::disallowed_methods,
    reason = "test seeding and read-back reach the node port and open units by hand"
)]

use std::sync::Arc;

use adapter_mem::fixture::{alice, bob, old_sketch, untitled};
use async_trait::async_trait;
use atta::{Viewer, WriteError};
use chrono::Utc;
use domain::{
    elements::{
        commission::{
            ChangelogEntry, ChangelogEntryKind, Commission, CommissionId, CommissionRole,
            CommissionStanding, CommissionTitle,
        },
        did::Did,
        level::Level,
        node::{NewNode, NodeId, NodeRow, NodeSegment, NodeType},
        text::StoredText,
    },
    ports::{
        AccountRepo, ActorIdentityWrites, BugReportWrites, ChangelogWrites, CharacterWrites,
        ColumnWrites, CommissionRepo, Database, New, NodeRepo, UnitOfWork, UserWrites,
        WorkflowWrites,
    },
};
use test_support::{
    pg_app::{StatementLog, pg_runtime, pg_runtime_recording},
    recording::{CallLog, RecordingDatabase},
    store_doubles::{ScriptedStanding, ScriptedStandingDatabase, StandingScript},
};

use super::*;
use crate::{
    den::{
        act::{Anchor, BornEntry},
        authority::{GrantKind, MINT_AS},
        gate::{DenApplied, DenRefusal},
        probe::{
            Probe, ProbeHandler, commission_path, draft, hand_built_create, hand_built_rename, name,
        },
    },
    test_world, transaction,
};

/// A DID nothing in the fixture holds a standing for.
const STRANGER: &str = "did:plc:den-write-stranger";

/// The probe's new name, which no log may carry.
const NEW_NAME: &str = "Renamed by the probe";

fn stranger() -> UserId {
    UserId::from(Did::from(STRANGER.to_owned()))
}

/// A commission id nothing is stored under.
fn made_up() -> CommissionId {
    CommissionId::from(uuid::Uuid::now_v7())
}

/// A node segment from a commission's id.
fn segment_of(commission: CommissionId) -> NodeSegment {
    NodeSegment::try_from(commission.to_string()).expect("an id is a segment")
}

/// The commission node for `commission`, named `title` at `level`, with its `products` entry.
pub(super) async fn seed_node(
    database: &dyn Database,
    commission: CommissionId,
    title: &str,
    level: Level,
) -> NodeId {
    let node = NewNode {
        id: NodeId::mint(),
        parent: None,
        segment: segment_of(commission),
        node_type: NodeType::Commission,
        level,
        name: StoredText::from(title.to_owned()),
    };
    let products = NewNode {
        id: NodeId::mint(),
        parent: Some(node.id),
        segment: NodeSegment::try_from("products").expect("a segment"),
        node_type: NodeType::CommissionProducts,
        level: Level::Private,
        name: StoredText::from("products".to_owned()),
    };
    let id = node.id;
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let mut rows = uow.nodes();
        rows.create(&node).await?;
        rows.create(&products).await?;
        Ok(())
    })
    .await
    .expect("the commission's node seeds");
    id
}

/// The top-level node row of `commission`, read back on a unit that keeps nothing.
pub(super) async fn top_row(database: &dyn Database, commission: CommissionId) -> Option<NodeRow> {
    let mut unit = database.begin().await.expect("begin");
    let row = unit
        .nodes()
        .at_top(&NodeType::Commission, &segment_of(commission))
        .await
        .expect("the node row reads");
    unit.rollback().await.expect("the read's unit ends");
    row
}

/// Every row directly under `parent`, read back on a unit that keeps nothing.
async fn rows_under(database: &dyn Database, parent: NodeId) -> Vec<NodeRow> {
    let mut unit = database.begin().await.expect("begin");
    let rows = unit
        .nodes()
        .children(&parent)
        .await
        .expect("the node rows read");
    unit.rollback().await.expect("the read's unit ends");
    rows
}

/// The stored name of `commission`'s node.
pub(super) async fn node_name(database: &dyn Database, commission: CommissionId) -> String {
    let row = top_row(database, commission)
        .await
        .expect("the commission has a node");
    row.name.as_str().to_owned()
}

/// Every changelog entry of `commission`.
async fn entries(ports: &Ports, commission: CommissionId) -> Vec<ChangelogEntry> {
    ports
        .changelog
        .entries(&commission)
        .await
        .expect("the changelog reads")
}

/// `actor`'s write of `act` through the use case.
pub(super) async fn write(
    ports: &Ports,
    actor: &UserId,
    act: DenAct,
) -> Result<WriteResult, DenWriteError> {
    let command = Command {
        actor: actor.clone(),
        act,
    };
    Den::from(ports).write(command, Utc::now()).await
}

/// The fixture world with "Untitled" and "Old sketch" given their nodes.
async fn world() -> test_world::World {
    let world = test_world::world().await;
    let database = &*world.ports.database;
    seed_node(database, untitled(), "Untitled", Level::Private).await;
    seed_node(database, old_sketch(), "Old sketch", Level::Private).await;
    world
}

/// Records `begin`, `commit` and `rollback` on `ports`' database from here on.
fn record_units(ports: &mut Ports) -> CallLog {
    let log = CallLog::default();
    let recording = RecordingDatabase::new(ports.database.clone(), log.clone());
    ports.database = Arc::new(recording);
    log
}

/// The owner's rename of "Untitled", needing the Owner.
fn owner_s_rename() -> Probe {
    Probe::rename(untitled(), NEW_NAME).needing(CommissionRole::Owner)
}

/// A draft the probe's handler hands back.
fn renamed_draft() -> ProbeHandler {
    let payload = serde_json::json!({ "probe": 1 });
    ProbeHandler::Draft(draft(ChangelogEntryKind::Renamed, payload))
}

/// Runs a hand-built act through a write host for `grant` on a fresh unit, as `viewer`,
/// committing only what the gate applied.
pub(super) async fn through_host(
    database: &dyn Database,
    viewer: Viewer,
    grant: Grant,
    act: Act<DenAct>,
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

/// An Owner's grant on `commission` to `actor`.
fn owner_grant(actor: &UserId, commission: CommissionId) -> Grant {
    let kind = GrantKind::Existing {
        commission,
        role: CommissionRole::Owner,
        level: Level::Private,
    };
    Grant::for_test(actor, kind)
}

// A stranger on a real commission and anyone on a made-up id get the one not-found, and no
// unit opens for either.
#[tokio::test]
async fn a_stranger_s_act_and_one_on_a_made_up_id_answer_the_same_not_found() {
    let mut world = world().await;
    let log = record_units(&mut world.ports);
    let cases = [
        ("a stranger", stranger(), untitled()),
        ("a made-up id", alice(), made_up()),
    ];

    for (label, actor, commission) in cases {
        let act = Probe::rename(commission, NEW_NAME).act();
        let written = write(&world.ports, &actor, act).await;

        assert!(
            matches!(written, Err(DenWriteError::NotFound)),
            "{label}: {written:?}"
        );
    }
    assert_eq!(log.events(), Vec::<&str>::new());
    assert_eq!(
        node_name(&*world.ports.database, untitled()).await,
        "Untitled"
    );
}

// On PostgreSQL, each runs exactly the one standing statement, and no `BEGIN`.
#[tokio::test]
async fn a_stranger_s_act_and_one_on_a_made_up_id_answer_the_same_not_found_on_postgres() {
    let statements = StatementLog::default();
    let pg = pg_runtime_recording(&statements).await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&*ports.database, "did:plc:den-write-pg-owner").await;
    let stranger = provision(&*ports.database, STRANGER).await;
    let cases = [
        ("a stranger", stranger, commission),
        ("a made-up id", owner, made_up()),
    ];

    for (label, actor, target) in cases {
        let mark = statements.mark();
        let act = Probe::rename(target, NEW_NAME).act();
        let written = write(&ports, &actor, act).await;

        let ran: Vec<String> = statements
            .since(mark)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert!(
            matches!(written, Err(DenWriteError::NotFound)),
            "{label}: {written:?}"
        );
        assert_eq!(ran, ["commission/standing"], "{label}");
    }
}

#[tokio::test]
async fn a_participant_below_the_act_s_role_is_forbidden_and_writes_nothing() {
    let mut world = world().await;
    let log = record_units(&mut world.ports);
    let act = owner_s_rename().handled(renamed_draft()).act();
    let before = entries(&world.ports, untitled()).await.len();

    let written = write(&world.ports, &bob(), act).await;

    assert!(
        matches!(written, Err(DenWriteError::Forbidden)),
        "{written:?}"
    );
    assert_eq!(log.events(), Vec::<&str>::new());
    assert_eq!(
        node_name(&*world.ports.database, untitled()).await,
        "Untitled"
    );
    assert_eq!(entries(&world.ports, untitled()).await.len(), before);
}

// Every answer the pool's verdict or the anchor's rule gives opens no unit.
#[tokio::test]
async fn a_refused_act_opens_no_unit() {
    let mut world = world().await;
    let log = record_units(&mut world.ports);
    let user_place = Place::User(crate::user::den::UserPlace::User(alice()));
    let cases = [
        (
            "a stranger",
            stranger(),
            Probe::rename(untitled(), NEW_NAME),
        ),
        ("a Participant below the need", bob(), owner_s_rename()),
        (
            "a rename at the top level",
            alice(),
            Probe::rename(untitled(), NEW_NAME).anchored(Anchor::TopLevel(TopLevel::Commission)),
        ),
        (
            "a place with no arm",
            alice(),
            Probe::rename(untitled(), NEW_NAME).anchored(Anchor::Thing(user_place)),
        ),
        (
            "a create in a top-level directory with no arm",
            alice(),
            Probe::create(NEW_NAME).anchored(Anchor::TopLevel(TopLevel::User)),
        ),
    ];

    for (label, actor, probe) in cases {
        let written = write(&world.ports, &actor, probe.act()).await;

        assert!(written.is_err(), "{label}");
        assert_eq!(log.events(), Vec::<&str>::new(), "{label}");
    }
}

// The pool's read said Owner; the locked read says otherwise, and the unit rolls back with the
// answer the pool would have given.
#[tokio::test]
async fn standing_lost_after_the_pool_read_refuses_inside_the_unit() {
    let owner_standing = CommissionStanding {
        role: Some(CommissionRole::Owner),
        level: Level::Private,
        archived: false,
    };
    let lost = [
        ("gone", None, "not_found"),
        (
            "no role",
            Some(CommissionStanding {
                role: None,
                ..owner_standing
            }),
            "not_found",
        ),
        (
            "a lower role",
            Some(CommissionStanding {
                role: Some(CommissionRole::Participant),
                ..owner_standing
            }),
            "forbidden",
        ),
    ];

    for (label, locked, answer) in lost {
        let mut world = world().await;
        let script = StandingScript::default();
        script.answer_standing(untitled(), alice(), Some(owner_standing));
        script.answer_standing_for_update(untitled(), alice(), locked);
        let pool = ScriptedStanding::new(world.ports.commissions.clone(), script.clone());
        let units = ScriptedStandingDatabase::new(world.ports.database.clone(), script);
        world.ports.commissions = Arc::new(pool);
        world.ports.database = Arc::new(units);
        let log = record_units(&mut world.ports);
        let act = owner_s_rename().handled(renamed_draft()).act();

        let written = write(&world.ports, &alice(), act).await;

        let class = match written {
            Err(DenWriteError::NotFound) => "not_found",
            Err(DenWriteError::Forbidden) => "forbidden",
            _ => "other",
        };
        assert_eq!(class, answer, "{label}");
        assert_eq!(log.events(), ["begin", "rollback"], "{label}");
        assert_eq!(
            node_name(&*world.ports.database, untitled()).await,
            "Untitled",
            "{label}"
        );
    }
}

#[tokio::test]
async fn the_locked_standing_s_level_wins() {
    let mut world = world().await;
    let at = |level| CommissionStanding {
        role: Some(CommissionRole::Owner),
        level,
        archived: false,
    };
    let script = StandingScript::default();
    script.answer_standing(untitled(), alice(), Some(at(Level::Listed)));
    script.answer_standing_for_update(untitled(), alice(), Some(at(Level::Public)));
    let pool = ScriptedStanding::new(world.ports.commissions.clone(), script.clone());
    let units = ScriptedStandingDatabase::new(world.ports.database.clone(), script);
    world.ports.commissions = Arc::new(pool);
    world.ports.database = Arc::new(units);

    let written = write(&world.ports, &alice(), owner_s_rename().act())
        .await
        .expect("the owner renames");

    assert_eq!(written.level, Level::Public);
}

#[tokio::test]
async fn a_walked_row_whose_type_differs_is_a_store_error_and_writes_nothing() {
    let world = world().await;
    let before = entries(&world.ports, untitled()).await.len();
    let act = owner_s_rename()
        .typed(DenType::CommissionProducts)
        .handled(renamed_draft())
        .act();

    let written = write(&world.ports, &alice(), act).await;

    assert!(
        matches!(written, Err(DenWriteError::Store(_))),
        "{written:?}"
    );
    assert_eq!(
        node_name(&*world.ports.database, untitled()).await,
        "Untitled"
    );
    assert_eq!(entries(&world.ports, untitled()).await.len(), before);
}

// A commission with no node row is the den's facts lying, not a miss.
#[tokio::test]
async fn a_walked_row_that_is_missing_is_a_store_error() {
    let world = test_world::world().await;
    let before = entries(&world.ports, untitled()).await.len();
    let act = owner_s_rename().handled(renamed_draft()).act();

    let written = write(&world.ports, &alice(), act).await;

    assert!(
        matches!(written, Err(DenWriteError::Store(_))),
        "{written:?}"
    );
    assert!(top_row(&*world.ports.database, untitled()).await.is_none());
    assert_eq!(entries(&world.ports, untitled()).await.len(), before);
}

// A grant naming another commission than the act: the write lands on the grant's, both when
// the den builds Atta's act and when the act names another path outright.
#[tokio::test]
async fn the_write_lands_on_the_granted_commission() {
    let world = world().await;
    let database = &*world.ports.database;
    let granted = old_sketch();
    let named = untitled();

    let built_grant = owner_grant(&alice(), granted);
    let act = Probe::rename(named, NEW_NAME)
        .needing(CommissionRole::Owner)
        .act();
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let result = written(uow, built_grant, act, Utc::now()).await?;
        Ok(result)
    })
    .await
    .expect("the granted write lands");

    assert_eq!(node_name(database, granted).await, NEW_NAME);
    assert_eq!(node_name(database, named).await, "Untitled");

    let host_grant = owner_grant(&alice(), granted);
    let probe = Probe::rename(named, NEW_NAME).needing(CommissionRole::Owner);
    let hand_built = hand_built_rename(commission_path(named), "Second name", probe);
    let applied = through_host(
        database,
        DenHost::viewer_of(&alice()),
        host_grant,
        hand_built,
    )
    .await
    .expect("the host applies it")
    .expect("the gate admits it");

    assert_eq!(applied.commission, granted);
    assert_eq!(node_name(database, granted).await, "Second name");
    assert_eq!(node_name(database, named).await, "Untitled");
}

// The builder takes the node's path from the grant, never from the act's own anchor.
#[test]
fn the_builder_paths_the_grant_s_commission() {
    let probe = Probe::rename(untitled(), NEW_NAME).needing(CommissionRole::Owner);
    let spec = probe.spec();
    let grant = owner_grant(&alice(), old_sketch());

    let built = build(spec, probe.act(), &grant).expect("the builder builds");

    let Shape::Rename { node, .. } = built.shape else {
        panic!("a rename builds a rename: {:?}", built.shape);
    };
    assert_eq!(node.path, commission_path(old_sketch()));
}

#[tokio::test]
async fn a_grant_of_the_wrong_kind_fails_closed() {
    let world = world().await;
    let database = &*world.ports.database;
    let viewer = DenHost::viewer_of(&alice());

    let existing_for_a_create = owner_grant(&alice(), untitled());
    let fresh = made_up();
    let create = hand_built_create(fresh, NEW_NAME, Probe::create(NEW_NAME));
    let created = through_host(database, viewer.clone(), existing_for_a_create, create)
        .await
        .expect("the host answers");

    assert_eq!(created, Err(DenRefusal::WrongGrant));
    assert!(top_row(database, fresh).await.is_none());

    let create_kind = GrantKind::Create {
        commission: New::from(untitled()),
    };
    let create_for_a_rename = Grant::for_test(&alice(), create_kind);
    let probe = Probe::rename(untitled(), NEW_NAME).needing(CommissionRole::Owner);
    let rename = hand_built_rename(commission_path(untitled()), NEW_NAME, probe);
    let renamed = through_host(database, viewer, create_for_a_rename, rename)
        .await
        .expect("the host answers");

    assert_eq!(renamed, Err(DenRefusal::WrongGrant));
    assert_eq!(node_name(database, untitled()).await, "Untitled");
}

#[tokio::test]
async fn a_rename_or_change_with_a_top_level_anchor_is_refused() {
    let mut world = world().await;
    let log = record_units(&mut world.ports);
    let top = Anchor::TopLevel(TopLevel::Commission);
    let probes = [
        Probe::rename(untitled(), NEW_NAME).anchored(top.clone()),
        Probe::change(untitled()).anchored(top),
    ];

    for probe in probes {
        let written = write(&world.ports, &alice(), probe.act()).await;

        assert!(
            matches!(
                written,
                Err(DenWriteError::Refused(RefusalClass::WrongGrant))
            ),
            "{written:?}"
        );
    }
    assert_eq!(log.events(), Vec::<&str>::new());
}

#[tokio::test]
async fn a_create_names_an_id_the_den_minted() {
    let world = world().await;
    let database = &*world.ports.database;

    let first = write(&world.ports, &stranger(), Probe::create(NEW_NAME).act())
        .await
        .expect("anyone signed in creates");
    let second = write(&world.ports, &stranger(), Probe::create(NEW_NAME).act())
        .await
        .expect("anyone signed in creates");

    let DenThing::Commission(first_id) = first.thing else {
        panic!("a create names a commission: {first:?}");
    };
    let DenThing::Commission(second_id) = second.thing else {
        panic!("a create names a commission: {second:?}");
    };
    assert_ne!(first_id, second_id);
    for minted in [first_id, second_id] {
        assert_eq!(uuid::Uuid::from(minted).get_version_num(), 7);
        let row = top_row(database, minted)
            .await
            .expect("its node is written");
        assert_eq!(row.name.as_str(), NEW_NAME);
    }
}

#[tokio::test]
async fn a_create_that_picks_a_wider_level_is_refused_in_0_2() {
    let world = world().await;
    let database = &*world.ports.database;

    for picked in [Level::Listed, Level::Public] {
        let planted = made_up();
        let act = Probe::create(NEW_NAME).picking(Some(picked)).act();
        let written = MINT_AS
            .scope(planted, write(&world.ports, &alice(), act))
            .await;

        assert!(
            matches!(
                written,
                Err(DenWriteError::Refused(RefusalClass::WiderThanPrivate))
            ),
            "{picked:?}: {written:?}"
        );
        assert!(top_row(database, planted).await.is_none(), "{picked:?}");
    }
}

#[tokio::test]
async fn a_create_writes_the_levels_atta_settled() {
    let world = world().await;
    let database = &*world.ports.database;
    let entries = vec![
        BornEntry {
            word: names::PRODUCTS,
            den_type: DenType::CommissionProducts,
            name: name("products"),
            level: None,
        },
        BornEntry {
            word: names::SLOTS,
            den_type: DenType::CommissionSlots,
            name: name("slots"),
            level: Some(Level::Private),
        },
    ];
    let act = Probe::create(NEW_NAME).holding(entries).act();

    let written = write(&world.ports, &alice(), act)
        .await
        .expect("the create lands");

    let DenThing::Commission(minted) = written.thing else {
        panic!("a create names a commission: {written:?}");
    };
    let node = top_row(database, minted)
        .await
        .expect("its node is written");
    let made = rows_under(database, node.id).await;
    let levels: Vec<Level> = std::iter::once(&node)
        .chain(&made)
        .map(|row| row.level)
        .collect();
    assert_eq!(written.level, Level::Private);
    assert_eq!(levels, [Level::Private; 3]);
}

#[tokio::test]
async fn the_entry_lands_on_the_granted_commission_with_the_grant_s_actor() {
    let world = world().await;
    let now = Utc::now();
    let act = owner_s_rename().handled(renamed_draft()).act();
    let command = Command {
        actor: alice(),
        act,
    };

    Den::from(&world.ports)
        .write(command, now)
        .await
        .expect("the owner renames");

    let stream = entries(&world.ports, untitled()).await;
    let entry = stream.last().expect("an entry landed");
    assert_eq!(entry.commission_id, untitled());
    assert_eq!(entry.actor_id, Some(alice()));
    assert_eq!(entry.kind, ChangelogEntryKind::Renamed);
    assert_eq!(entry.payload, serde_json::json!({ "probe": 1 }));
    assert_eq!(entry.created_at, now);
}

#[tokio::test]
async fn the_owner_s_act_lands_whole() {
    let world = world().await;
    let before = entries(&world.ports, untitled()).await.len();

    let written = write(
        &world.ports,
        &alice(),
        owner_s_rename().handled(renamed_draft()).act(),
    )
    .await
    .expect("the owner renames");

    let expected_name = DenName::try_from(NEW_NAME).expect("a name");
    assert_eq!(written.name, Some(expected_name));
    assert_eq!(written.thing, DenThing::Commission(untitled()));
    assert_eq!(
        node_name(&*world.ports.database, untitled()).await,
        NEW_NAME
    );
    assert_eq!(entries(&world.ports, untitled()).await.len(), before + 1);
}

#[tokio::test]
async fn the_owner_s_act_lands_whole_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&*ports.database, "did:plc:den-write-pg-whole").await;
    let before = entries(&ports, commission).await.len();
    let act = Probe::rename(commission, NEW_NAME)
        .needing(CommissionRole::Owner)
        .handled(renamed_draft())
        .act();

    write(&ports, &owner, act).await.expect("the owner renames");

    assert_eq!(node_name(&*ports.database, commission).await, NEW_NAME);
    let stream = entries(&ports, commission).await;
    assert_eq!(stream.len(), before + 1);
    let entry = stream.last().expect("an entry landed");
    assert_eq!(entry.actor_id, Some(owner));
}

#[tokio::test]
async fn a_failed_entry_append_leaves_the_node_unrenamed() {
    let mut world = world().await;
    let refusing = RefusingChangelogDatabase(world.ports.database.clone());
    world.ports.database = Arc::new(refusing);
    let act = owner_s_rename().handled(renamed_draft()).act();

    let written = write(&world.ports, &alice(), act).await;

    assert!(
        matches!(written, Err(DenWriteError::Store(_))),
        "{written:?}"
    );
    assert_eq!(
        node_name(&*world.ports.database, untitled()).await,
        "Untitled"
    );
}

#[tokio::test]
async fn a_failed_handler_writes_neither_row_nor_entry() {
    let world = world().await;
    let before = entries(&world.ports, untitled()).await.len();
    let handlers = [
        (ProbeHandler::Refuse, "refused"),
        (ProbeHandler::Fail, "store"),
    ];

    for (handler, answer) in handlers {
        let act = owner_s_rename().handled(handler).act();

        let written = write(&world.ports, &alice(), act).await;

        let class = match written {
            Err(DenWriteError::Refused(RefusalClass::TypeRefused)) => "refused",
            Err(DenWriteError::Store(_)) => "store",
            _ => "other",
        };
        assert_eq!(class, answer);
        assert_eq!(
            node_name(&*world.ports.database, untitled()).await,
            "Untitled"
        );
        assert_eq!(entries(&world.ports, untitled()).await.len(), before);
    }
}

/// A create holding one entry, whose minted id is planted on `taken`.
async fn create_on(
    ports: &Ports,
    actor: &UserId,
    taken: CommissionId,
) -> Result<WriteResult, DenWriteError> {
    let entries = vec![BornEntry {
        word: names::PRODUCTS,
        den_type: DenType::CommissionProducts,
        name: name("products"),
        level: None,
    }];
    let act = Probe::create(NEW_NAME).holding(entries).act();
    MINT_AS.scope(taken, write(ports, actor, act)).await
}

#[tokio::test]
async fn a_create_on_a_taken_segment_leaves_as_an_error_and_keeps_nothing() {
    let world = world().await;
    let database = &*world.ports.database;
    let before = top_row(database, untitled()).await.expect("seeded");

    let written = create_on(&world.ports, &alice(), untitled()).await;

    assert!(
        matches!(
            written,
            Err(DenWriteError::Refused(RefusalClass::SegmentTaken))
        ),
        "{written:?}"
    );
    let after = top_row(database, untitled()).await.expect("still there");
    assert!(after == before);
    assert_eq!(rows_under(database, after.id).await.len(), 1);
}

#[tokio::test]
async fn a_create_on_a_taken_segment_leaves_as_an_error_and_keeps_nothing_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&*ports.database, "did:plc:den-write-pg-taken").await;
    let before = top_row(&*ports.database, commission).await.expect("seeded");

    let written = create_on(&ports, &owner, commission).await;

    assert!(
        matches!(
            written,
            Err(DenWriteError::Refused(RefusalClass::SegmentTaken))
        ),
        "{written:?}"
    );
    let after = top_row(&*ports.database, commission)
        .await
        .expect("still there");
    assert!(after == before);
    assert_eq!(rows_under(&*ports.database, after.id).await.len(), 1);
}

// Archiving refuses nothing yet: a later ruling flips this one test.
#[tokio::test]
async fn an_archived_commission_refuses_nothing_yet() {
    let world = world().await;
    let act = Probe::rename(old_sketch(), NEW_NAME)
        .needing(CommissionRole::Owner)
        .act();

    write(&world.ports, &alice(), act)
        .await
        .expect("the owner renames an archived commission");

    assert_eq!(
        node_name(&*world.ports.database, old_sketch()).await,
        NEW_NAME
    );
}

/// The user `did`, recognized.
pub(super) async fn provision(database: &dyn Database, did: &str) -> UserId {
    let did = Did::from(did.to_owned());
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let user = uow.users().provision(&did).await?;
        Ok(user.id)
    })
    .await
    .expect("the user is recognized")
}

/// The owner `owner_did`, recognized, and a commission of theirs with its node.
pub(super) async fn pg_seed(database: &dyn Database, owner_did: &str) -> (UserId, CommissionId) {
    let owner = provision(database, owner_did).await;
    let title: CommissionTitle = "Probed piece".parse().expect("a title");
    let commission = Commission::create(title, owner.clone(), Utc::now(), None);
    let id = commission.id;
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        uow.commissions().create(&commission).await?;
        Ok(())
    })
    .await
    .expect("the commission seeds");
    seed_node(database, id, "Probed piece", Level::Private).await;
    (owner, id)
}

/// A [`Database`] whose units refuse every changelog append.
struct RefusingChangelogDatabase(Arc<dyn Database>);

#[async_trait]
impl Database for RefusingChangelogDatabase {
    async fn begin(&self) -> anyhow::Result<Box<dyn UnitOfWork>> {
        let inner = self.0.begin().await?;
        Ok(Box::new(RefusingChangelogUnit(inner)))
    }
}

/// A real unit whose `changelog()` refuses; everything else delegates.
struct RefusingChangelogUnit(Box<dyn UnitOfWork>);

/// A changelog that refuses every append.
struct RefusingChangelog;

#[async_trait]
impl ChangelogWrites for RefusingChangelog {
    async fn append(
        &mut self,
        _entry: &domain::elements::commission::NewChangelogEntry,
    ) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("the changelog refuses"))
    }
}

#[async_trait]
impl UnitOfWork for RefusingChangelogUnit {
    fn accounts(&mut self) -> Box<dyn AccountRepo + '_> {
        self.0.accounts()
    }

    fn commissions(&mut self) -> Box<dyn CommissionRepo + '_> {
        self.0.commissions()
    }

    fn changelog(&mut self) -> Box<dyn ChangelogWrites + '_> {
        Box::new(RefusingChangelog)
    }

    fn users(&mut self) -> Box<dyn UserWrites + '_> {
        self.0.users()
    }

    fn actor_identities(&mut self) -> Box<dyn ActorIdentityWrites + '_> {
        self.0.actor_identities()
    }

    fn workflows(&mut self) -> Box<dyn WorkflowWrites + '_> {
        self.0.workflows()
    }

    fn columns(&mut self) -> Box<dyn ColumnWrites + '_> {
        self.0.columns()
    }

    fn characters(&mut self) -> Box<dyn CharacterWrites + '_> {
        self.0.characters()
    }

    fn bug_reports(&mut self) -> Box<dyn BugReportWrites + '_> {
        self.0.bug_reports()
    }

    fn nodes(&mut self) -> Box<dyn NodeRepo + '_> {
        self.0.nodes()
    }

    async fn commit(self: Box<Self>) -> anyhow::Result<()> {
        self.0.commit().await
    }

    async fn rollback(self: Box<Self>) -> anyhow::Result<()> {
        self.0.rollback().await
    }
}
