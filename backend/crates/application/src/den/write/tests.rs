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
            ChangelogEntry, ChangelogEntryKind, CommissionId, CommissionRole, CommissionStanding,
            CommissionState, DeadlineStatus, DirectionStatus, LifecycleStep, LockedStanding,
            StoredDispute,
        },
        did::Did,
        level::Level,
        node::{NodeId, NodeRow, NodeSegment, NodeType},
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
    commission::{Commissions, create},
    den::{
        authority::{GrantKind, MINT_AS},
        gate::{DenApplied, DenRefusal},
        test_acts::{
            FieldCall, clear_deadline_act, clear_delayed_act, clear_direction_act,
            clear_dispute_act, commission_path, create_act, field_call, fields_of,
            hand_built_create, hand_built_create_of, hand_built_rename, hand_built_rename_of,
            move_act, raise_dispute_act, rename_act, set_deadline_act, set_delayed_act,
            set_direction_act, state_at, state_disputed_from, state_with,
        },
    },
    test_world, transaction,
};

/// A DID nothing in the fixture holds a standing for.
const STRANGER: &str = "did:plc:den-write-stranger";

/// The rename's new name, which no log may carry.
const NEW_NAME: &str = "Renamed through the gate";

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

/// The node of `commission`, which must have one.
async fn node_of(database: &dyn Database, commission: CommissionId) -> NodeId {
    let row = top_row(database, commission)
        .await
        .expect("the commission has a node");
    row.id
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

/// The kinds of `commission`'s entries, in stream order.
async fn kinds(ports: &Ports, commission: CommissionId) -> Vec<ChangelogEntryKind> {
    let stream = entries(ports, commission).await;
    stream.iter().map(|entry| entry.kind).collect()
}

/// `actor`'s write of `act` through the use case.
pub(super) async fn write(
    ports: &Ports,
    actor: &UserId,
    act: DenAct,
) -> Result<WriteResult, DenWriteError> {
    let command = Command {
        actor: Actor::User(actor.clone()),
        act,
    };
    Den::from(ports).write(command, Utc::now()).await
}

/// The fixture world, whose "Untitled" and "Old sketch" stand bound to their nodes.
async fn world() -> test_world::World {
    test_world::world().await
}

/// Records `begin`, `commit` and `rollback` on `ports`' database from here on.
fn record_units(ports: &mut Ports) -> CallLog {
    let log = CallLog::default();
    let recording = RecordingDatabase::new(ports.database.clone(), log.clone());
    ports.database = Arc::new(recording);
    log
}

/// The owner's rename of "Untitled" to [`NEW_NAME`].
fn owner_s_rename() -> DenAct {
    rename_act(untitled(), NEW_NAME)
}

/// The Owner's live Private standing.
fn owner_standing() -> CommissionStanding {
    CommissionStanding {
        role: Some(CommissionRole::Owner),
        level: Level::Private,
        archived: false,
    }
}

/// `standing`, locked on the node `node`.
fn locked(standing: CommissionStanding, node: NodeId) -> LockedStanding {
    LockedStanding { standing, node }
}

/// Scripts `world`'s pool to answer `pooled` and its units `locked` for alice on `commission`.
fn script_standing(
    world: &mut test_world::World,
    commission: CommissionId,
    pooled: Option<CommissionStanding>,
    locked: Option<LockedStanding>,
) {
    let script = StandingScript::default();
    script.answer_standing(commission, alice(), pooled);
    script.answer_standing_for_update(commission, alice(), locked);
    let pool = ScriptedStanding::new(world.ports.commissions.clone(), script.clone());
    let units = ScriptedStandingDatabase::new(world.ports.database.clone(), script);
    world.ports.commissions = Arc::new(pool);
    world.ports.database = Arc::new(units);
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
    let writer = den_writer(viewer);
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

/// An Owner's grant on `commission` to `actor`, bound to `node`, carrying `state`.
fn owner_grant_with(
    actor: &UserId,
    commission: CommissionId,
    node: NodeId,
    state: CommissionState,
) -> Grant {
    let kind = GrantKind::Existing {
        commission,
        role: CommissionRole::Owner,
        level: Level::Private,
        node,
        state,
    };
    Grant::for_test(actor, kind)
}

/// An Owner's grant on `commission` to `actor`, bound to `node`, its state at Draft.
fn owner_grant(actor: &UserId, commission: CommissionId, node: NodeId) -> Grant {
    owner_grant_with(actor, commission, node, state_at(LifecycleStep::Draft))
}

/// An Owner's grant on `commission` to `actor`, bound to its stored node.
async fn bound_owner_grant(
    database: &dyn Database,
    actor: &UserId,
    commission: CommissionId,
) -> Grant {
    let node = node_of(database, commission).await;
    owner_grant(actor, commission, node)
}

// A grant's and an act's `Debug` print neither the actor's DID nor the name the act carries.
#[test]
fn a_grant_and_an_act_debug_print_no_did_or_name() {
    let actor = stranger();
    let grant = owner_grant(&actor, made_up(), NodeId::mint());
    let act = owner_s_rename();

    let printed = format!("{grant:?} {act:?}");

    assert!(!printed.contains(STRANGER), "a DID in {printed}");
    assert!(!printed.contains(NEW_NAME), "a name in {printed}");
}

// A stranger on a real commission and anyone on a made-up id get the one not-found, and no
// unit opens for either.
#[tokio::test]
async fn a_stranger_s_rename_and_one_on_a_made_up_id_answer_the_same_not_found() {
    let mut world = world().await;
    let log = record_units(&mut world.ports);
    let cases = [
        ("a stranger", stranger(), untitled()),
        ("a made-up id", alice(), made_up()),
    ];

    for (label, actor, commission) in cases {
        let act = rename_act(commission, NEW_NAME);
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
async fn a_stranger_s_rename_and_one_on_a_made_up_id_answer_the_same_not_found_on_postgres() {
    let statements = StatementLog::default();
    let pg = pg_runtime_recording(&statements).await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, "did:plc:den-write-pg-owner").await;
    let stranger = provision(&*ports.database, STRANGER).await;
    let cases = [
        ("a stranger", stranger, commission),
        ("a made-up id", owner, made_up()),
    ];

    for (label, actor, target) in cases {
        let mark = statements.mark();
        let act = rename_act(target, NEW_NAME);
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

// Rename needs the Owner: bob, a Participant of "Untitled", is forbidden before any unit opens.
#[tokio::test]
async fn a_participant_s_rename_is_forbidden_and_writes_nothing() {
    let mut world = world().await;
    let log = record_units(&mut world.ports);
    let before = entries(&world.ports, untitled()).await.len();

    let written = write(&world.ports, &bob(), owner_s_rename()).await;

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

// Every refusal the pool's verdict gives a rename comes before any unit opens.
#[tokio::test]
async fn a_stranger_s_rename_opens_no_unit() {
    let mut world = world().await;
    let log = record_units(&mut world.ports);
    let cases = [
        ("a stranger", stranger(), untitled()),
        ("a made-up id", alice(), made_up()),
        ("a Participant below the need", bob(), untitled()),
    ];

    for (label, actor, commission) in cases {
        let written = write(&world.ports, &actor, rename_act(commission, NEW_NAME)).await;

        assert!(written.is_err(), "{label}");
        assert_eq!(log.events(), Vec::<&str>::new(), "{label}");
    }
}

// The pool's read said Owner; the locked read says otherwise, and the unit rolls back with the
// answer the pool would have given.
#[tokio::test]
async fn standing_lost_after_the_pool_read_refuses_inside_the_unit() {
    let lost = [
        ("gone", None, "not_found"),
        (
            "no role",
            Some(CommissionStanding {
                role: None,
                ..owner_standing()
            }),
            "not_found",
        ),
        (
            "a lower role",
            Some(CommissionStanding {
                role: Some(CommissionRole::Participant),
                ..owner_standing()
            }),
            "forbidden",
        ),
    ];

    for (label, locked_standing, answer) in lost {
        let mut world = world().await;
        let node = node_of(&*world.ports.database, untitled()).await;
        let locked_answer = locked_standing.map(|standing| locked(standing, node));
        script_standing(
            &mut world,
            untitled(),
            Some(owner_standing()),
            locked_answer,
        );
        let log = record_units(&mut world.ports);

        let written = write(&world.ports, &alice(), owner_s_rename()).await;

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
    let node = node_of(&*world.ports.database, untitled()).await;
    let at = |level| CommissionStanding {
        level,
        ..owner_standing()
    };
    let locked_public = locked(at(Level::Public), node);
    script_standing(
        &mut world,
        untitled(),
        Some(at(Level::Listed)),
        Some(locked_public),
    );

    let written = write(&world.ports, &alice(), owner_s_rename())
        .await
        .expect("the owner renames");

    assert_eq!(written.level, Level::Public);
}

// A rename built by hand onto a node of another type than the walked row's: Atta passes it,
// and the gate's walk refuses it as a store failure.
#[tokio::test]
async fn a_walked_row_whose_type_differs_is_a_store_error_and_writes_nothing() {
    let world = world().await;
    let database = &*world.ports.database;
    let before = entries(&world.ports, untitled()).await.len();
    let grant = bound_owner_grant(database, &alice(), untitled()).await;
    let act = hand_built_rename_of(
        commission_path(untitled()),
        DenType::CommissionProducts,
        NEW_NAME,
        owner_s_rename(),
    );

    let applied = through_host(database, DenHost::viewer_of(&alice()), grant, act).await;

    assert!(matches!(applied, Err(WriteError::Host(_))), "{applied:?}");
    assert_eq!(node_name(database, untitled()).await, "Untitled");
    assert_eq!(entries(&world.ports, untitled()).await.len(), before);
}

// A standing on a commission with no node row is the den's facts lying, not a miss.
#[tokio::test]
async fn a_walked_row_that_is_missing_is_a_store_error() {
    let mut world = world().await;
    let nowhere = made_up();
    let locked_nowhere = locked(owner_standing(), NodeId::mint());
    script_standing(
        &mut world,
        nowhere,
        Some(owner_standing()),
        Some(locked_nowhere),
    );

    let written = write(&world.ports, &alice(), rename_act(nowhere, NEW_NAME)).await;

    assert!(
        matches!(written, Err(DenWriteError::Store(_))),
        "{written:?}"
    );
    assert!(top_row(&*world.ports.database, nowhere).await.is_none());
    assert_eq!(entries(&world.ports, nowhere).await.len(), 0);
}

// The locked read binds the grant to a node other than the commission's own: the walk finds
// the commission's row, refuses it as not the bound node, and nothing is written.
#[tokio::test]
async fn a_rename_whose_walked_row_is_not_the_bound_node_is_a_store_error() {
    let mut world = world().await;
    let before = entries(&world.ports, untitled()).await.len();
    let elsewhere = node_of(&*world.ports.database, old_sketch()).await;
    let locked_elsewhere = locked(owner_standing(), elsewhere);
    script_standing(
        &mut world,
        untitled(),
        Some(owner_standing()),
        Some(locked_elsewhere),
    );

    let written = write(&world.ports, &alice(), owner_s_rename()).await;

    assert!(
        matches!(written, Err(DenWriteError::Store(_))),
        "{written:?}"
    );
    let database = &*world.ports.database;
    assert_eq!(node_name(database, untitled()).await, "Untitled");
    assert_eq!(node_name(database, old_sketch()).await, "Old sketch");
    assert_eq!(entries(&world.ports, untitled()).await.len(), before);
}

// A grant naming another commission than the act: the rename lands on the grant's, both when
// the den builds Atta's act and when the act names another path outright.
#[tokio::test]
async fn the_rename_lands_on_the_granted_commission() {
    let world = world().await;
    let database = &*world.ports.database;
    let granted = old_sketch();
    let named = untitled();

    let built_grant = bound_owner_grant(database, &alice(), granted).await;
    let act = rename_act(named, NEW_NAME);
    transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let result = written(uow, built_grant, act, Utc::now()).await?;
        Ok(result)
    })
    .await
    .expect("the granted rename lands");

    assert_eq!(node_name(database, granted).await, NEW_NAME);
    assert_eq!(node_name(database, named).await, "Untitled");

    let host_grant = bound_owner_grant(database, &alice(), granted).await;
    let hand_built = hand_built_rename(commission_path(named), named, "Second name");
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
    let act = owner_s_rename();
    let spec = act.spec();
    let grant = owner_grant(&alice(), old_sketch(), NodeId::mint());

    let built = build(spec, act, &Target::from(&grant)).expect("the builder builds");

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

    let existing_for_a_create = bound_owner_grant(database, &alice(), untitled()).await;
    let fresh = made_up();
    let create = hand_built_create(fresh, NEW_NAME);
    let created = through_host(database, viewer.clone(), existing_for_a_create, create)
        .await
        .expect("the host answers");

    assert!(
        matches!(created, Err(DenRefusal::WrongGrant)),
        "{created:?}"
    );
    assert!(top_row(database, fresh).await.is_none());

    let create_kind = GrantKind::Create {
        commission: New::from(untitled()),
    };
    let create_for_a_rename = Grant::for_test(&alice(), create_kind);
    let rename = hand_built_rename(commission_path(untitled()), untitled(), NEW_NAME);
    let renamed = through_host(database, viewer, create_for_a_rename, rename)
        .await
        .expect("the host answers");

    assert!(
        matches!(renamed, Err(DenRefusal::WrongGrant)),
        "{renamed:?}"
    );
    assert_eq!(node_name(database, untitled()).await, "Untitled");
}

#[tokio::test]
async fn a_create_names_an_id_the_den_minted() {
    let world = world().await;
    let database = &*world.ports.database;

    let first = write(&world.ports, &alice(), create_act(NEW_NAME))
        .await
        .expect("alice creates");
    let second = write(&world.ports, &bob(), create_act(NEW_NAME))
        .await
        .expect("bob creates");

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
        assert_eq!(row.segment.as_ref(), minted.to_string());
        assert_eq!(row.name.as_str(), NEW_NAME);
    }
}

// The real create's body, in an act built by hand that picks a level wider than Private: Atta
// passes it under the Public `/commission`, and the gate refuses it until 0.6.
#[tokio::test]
async fn a_create_that_picks_a_wider_level_is_refused_in_0_2() {
    let world = world().await;
    let database = &*world.ports.database;

    for picked in [atta::Level::Listed, atta::Level::Public] {
        let planted = made_up();
        let kind = GrantKind::Create {
            commission: New::from(planted),
        };
        let grant = Grant::for_test(&alice(), kind);
        let act = hand_built_create_of(
            planted,
            NEW_NAME,
            DenType::Commission,
            Some(picked),
            create_act(NEW_NAME),
        );

        let applied = through_host(database, DenHost::viewer_of(&alice()), grant, act)
            .await
            .expect("Atta passes it");

        assert!(
            matches!(applied, Err(DenRefusal::WiderThanPrivate)),
            "{picked:?}: {applied:?}"
        );
        assert!(top_row(database, planted).await.is_none(), "{picked:?}");
        let stored = world
            .ports
            .commissions
            .find(&planted)
            .await
            .expect("the store reads");
        assert!(stored.is_none(), "{picked:?}");
    }
}

/// The six entries a commission is made with: each word, its stored type, its name.
const SIX_ENTRIES: [(&str, NodeType); 6] = [
    ("attachments", NodeType::CommissionAttachments),
    ("brief", NodeType::CommissionBrief),
    ("changelog", NodeType::CommissionChangelog),
    ("participants", NodeType::CommissionParticipants),
    ("products", NodeType::CommissionProducts),
    ("slots", NodeType::CommissionSlots),
];

#[tokio::test]
async fn a_create_writes_its_node_and_six_entries_private() {
    let world = world().await;
    let database = &*world.ports.database;

    let written = write(&world.ports, &alice(), create_act(NEW_NAME))
        .await
        .expect("the create lands");

    let DenThing::Commission(minted) = written.thing else {
        panic!("a create names a commission: {written:?}");
    };
    let node = top_row(database, minted)
        .await
        .expect("its node is written");
    let mut made: Vec<(String, NodeType, String, Level)> = rows_under(database, node.id)
        .await
        .into_iter()
        .map(|row| {
            let segment = row.segment.as_ref().to_owned();
            let name = row.name.as_str().to_owned();
            (segment, row.node_type, name, row.level)
        })
        .collect();
    made.sort_by(|left, right| left.0.cmp(&right.0));
    let expected: Vec<(String, NodeType, String, Level)> = SIX_ENTRIES
        .iter()
        .map(|(word, node_type)| {
            let word = (*word).to_owned();
            (word.clone(), *node_type, word, Level::Private)
        })
        .collect();
    assert_eq!(node.node_type, NodeType::Commission);
    assert_eq!(node.level, Level::Private);
    assert_eq!(node.name.as_str(), NEW_NAME);
    assert_eq!(written.level, Level::Private);
    assert_eq!(made, expected);
}

#[tokio::test]
async fn the_entry_lands_on_the_granted_commission_with_the_grant_s_actor() {
    let world = world().await;
    let now = Utc::now();
    let command = Command {
        actor: Actor::User(alice()),
        act: owner_s_rename(),
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
    assert_eq!(entry.payload, serde_json::json!({}));
    assert_eq!(entry.note, None);
    assert_eq!(entry.created_at, now);
}

// A rename to the name the node already holds lands, answers that name, and writes no entry.
#[tokio::test]
async fn a_no_op_rename_writes_no_entry() {
    let world = world().await;
    let before = kinds(&world.ports, untitled()).await;

    let written = write(&world.ports, &alice(), rename_act(untitled(), "Untitled"))
        .await
        .expect("the owner renames to the same name");

    let expected_name = DenName::try_from("Untitled").expect("a name");
    assert_eq!(written.name, Some(expected_name));
    assert_eq!(written.thing, DenThing::Commission(untitled()));
    assert_eq!(kinds(&world.ports, untitled()).await, before);
    assert_eq!(
        node_name(&*world.ports.database, untitled()).await,
        "Untitled"
    );
}

// On PostgreSQL, a rename to the stored name reads its node once and writes no node row and
// no entry.
#[tokio::test]
async fn a_no_op_rename_writes_nothing_on_postgres() {
    let statements = StatementLog::default();
    let pg = pg_runtime_recording(&statements).await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, "did:plc:den-write-pg-no-op").await;
    let mark = statements.mark();

    write(&ports, &owner, rename_act(commission, PG_TITLE))
        .await
        .expect("the owner renames to the same name");

    let written: Vec<&str> = statements
        .since(mark)
        .iter()
        .filter_map(|statement| statement.query_name())
        .filter(|name| name.starts_with("node/") || name.starts_with("changelog/"))
        .collect();
    assert_eq!(written, ["node/at_top"]);
    assert_eq!(
        kinds(&ports, commission).await,
        [ChangelogEntryKind::Created]
    );
}

// A rename to a new name writes exactly one entry, `renamed`.
#[tokio::test]
async fn a_real_rename_writes_one_renamed_entry() {
    let world = world().await;
    let before = kinds(&world.ports, untitled()).await;

    write(&world.ports, &alice(), owner_s_rename())
        .await
        .expect("the owner renames");

    let after = kinds(&world.ports, untitled()).await;
    let (kept, added) = after.split_at(before.len());
    assert_eq!(kept, before.as_slice());
    assert_eq!(added, [ChangelogEntryKind::Renamed]);
}

#[tokio::test]
async fn the_owner_s_rename_lands_whole() {
    let world = world().await;
    let before = entries(&world.ports, untitled()).await.len();

    let written = write(&world.ports, &alice(), owner_s_rename())
        .await
        .expect("the owner renames");

    let expected_name = DenName::try_from(NEW_NAME).expect("a name");
    assert_eq!(written.name, Some(expected_name));
    assert_eq!(written.thing, DenThing::Commission(untitled()));
    let ArmAnswer::Commission(answer) = written.answer;
    assert_eq!(answer.id, untitled());
    assert_eq!(
        node_name(&*world.ports.database, untitled()).await,
        NEW_NAME
    );
    assert_eq!(entries(&world.ports, untitled()).await.len(), before + 1);
}

#[tokio::test]
async fn the_owner_s_rename_lands_whole_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, "did:plc:den-write-pg-whole").await;
    let before = entries(&ports, commission).await.len();

    let written = write(&ports, &owner, rename_act(commission, NEW_NAME))
        .await
        .expect("the owner renames");

    let expected_name = DenName::try_from(NEW_NAME).expect("a name");
    assert_eq!(written.name, Some(expected_name));
    assert_eq!(node_name(&*ports.database, commission).await, NEW_NAME);
    let stream = entries(&ports, commission).await;
    assert_eq!(stream.len(), before + 1);
    let entry = stream.last().expect("an entry landed");
    assert_eq!(entry.actor_id, Some(owner));
    assert_eq!(entry.kind, ChangelogEntryKind::Renamed);
}

#[tokio::test]
async fn a_failed_entry_append_leaves_the_node_unrenamed() {
    let mut world = world().await;
    let refusing = RefusingChangelogDatabase(world.ports.database.clone());
    world.ports.database = Arc::new(refusing);

    let written = write(&world.ports, &alice(), owner_s_rename()).await;

    assert!(
        matches!(written, Err(DenWriteError::Store(_))),
        "{written:?}"
    );
    assert_eq!(
        node_name(&*world.ports.database, untitled()).await,
        "Untitled"
    );
}

/// The real create, its minted id planted on `taken`.
pub(super) async fn create_on(
    ports: &Ports,
    actor: &UserId,
    taken: CommissionId,
) -> Result<WriteResult, DenWriteError> {
    MINT_AS
        .scope(taken, write(ports, actor, create_act(NEW_NAME)))
        .await
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
    assert_eq!(rows_under(database, after.id).await.len(), 6);
}

#[tokio::test]
async fn a_create_on_a_taken_segment_leaves_as_an_error_and_keeps_nothing_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, "did:plc:den-write-pg-taken").await;
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
    assert_eq!(rows_under(&*ports.database, after.id).await.len(), 6);
}

// Archiving refuses nothing yet: a later ruling flips this one test.
#[tokio::test]
async fn an_archived_commission_refuses_nothing_yet() {
    let world = world().await;
    let act = rename_act(old_sketch(), NEW_NAME);

    write(&world.ports, &alice(), act)
        .await
        .expect("the owner renames an archived commission");

    assert_eq!(
        node_name(&*world.ports.database, old_sketch()).await,
        NEW_NAME
    );
}

/// The lifecycle step `commission` holds.
async fn step_of(ports: &Ports, commission: CommissionId) -> LifecycleStep {
    let stored = ports
        .commissions
        .find(&commission)
        .await
        .expect("the store reads")
        .expect("the commission is stored");
    stored.lifecycle_step
}

/// `actor`'s move of `commission` to `to` through the Den's write.
async fn move_by(
    ports: &Ports,
    actor: &UserId,
    commission: CommissionId,
    to: LifecycleStep,
) -> Result<WriteResult, DenWriteError> {
    write(ports, actor, move_act(commission, to)).await
}

/// `actor`'s move of `commission` to `to` through the move use case, which hands its act to
/// the Den's write and holds no unit of its own.
async fn move_through_the_use_case(
    ports: &Ports,
    actor: &UserId,
    commission: CommissionId,
    to: LifecycleStep,
) -> Result<(), crate::commission::CommissionError> {
    let command = crate::commission::lifecycle::move_to::Command {
        actor_id: actor.clone(),
        commission_id: commission,
        to,
    };
    Commissions::new(ports)
        .lifecycle()
        .move_to(command, Utc::now())
        .await?;
    Ok(())
}

/// `actor`'s move of `commission`, held at Active, to Completed under a grant whose state says
/// Draft: the rule checks a move from Draft, the compare-and-set matches no row, and the act
/// fails, leaving the step and the entries as they were.
async fn a_stale_move_fails(ports: &Ports, actor: &UserId, commission: CommissionId) {
    let database = &*ports.database;
    let before = entries(ports, commission).await.len();
    let node = node_of(database, commission).await;
    let stale = owner_grant_with(actor, commission, node, state_at(LifecycleStep::Draft));
    let act = move_act(commission, LifecycleStep::Completed);

    let moved = transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let result = written(uow, stale, act, Utc::now()).await?;
        Ok(result)
    })
    .await;

    assert!(moved.is_err(), "{moved:?}");
    assert_eq!(step_of(ports, commission).await, LifecycleStep::Active);
    assert_eq!(entries(ports, commission).await.len(), before);
}

/// A deadline far ahead, so no act here meets Late.
fn ahead() -> chrono::DateTime<Utc> {
    "2099-03-01T00:00:00Z".parse().expect("a timestamp")
}

/// A later deadline than [`ahead`].
fn further_ahead() -> chrono::DateTime<Utc> {
    "2099-04-01T00:00:00Z".parse().expect("a timestamp")
}

/// `actor`'s `act` on `commission`, under an Owner's grant whose state is `stale`: its rule
/// checks a change from a value the row no longer holds, the compare-and-set matches no row,
/// and the act fails, leaving the fields and the entries as they were.
async fn a_stale_field_act_fails(
    ports: &Ports,
    actor: &UserId,
    commission: CommissionId,
    stale: CommissionState,
    act: DenAct,
) {
    let database = &*ports.database;
    let before = fields_of(ports, commission).await;
    let logged = entries(ports, commission).await.len();
    let node = node_of(database, commission).await;
    let grant = owner_grant_with(actor, commission, node, stale);
    let label = format!("{act:?}");

    let written = transaction(database, async move |uow: &mut dyn UnitOfWork| {
        let result = written(uow, grant, act, Utc::now()).await?;
        Ok(result)
    })
    .await;

    assert!(written.is_err(), "{label}: {written:?}");
    assert_eq!(fields_of(ports, commission).await, before, "{label}");
    assert_eq!(entries(ports, commission).await.len(), logged, "{label}");
}

/// Each field act's stale act on `commission`, `actor`'s: a direction set from no status and a
/// clear of another status than the row holds; a deadline set from no deadline and a clear of
/// another deadline than the row holds; a Delayed flag raised from no stored flag while the row
/// holds one, and taken down while the row holds none.
async fn each_stale_field_act_fails(ports: &Ports, actor: &UserId, commission: CommissionId) {
    let held = [
        FieldCall::SetDirection(DirectionStatus::WaitingForInput),
        FieldCall::SetDeadline(ahead()),
        FieldCall::SetDelayed,
    ];
    for call in held {
        field_call(ports, actor, commission, call, Utc::now())
            .await
            .expect("the field lands");
    }
    let step = fields_of(ports, commission).await.lifecycle_step().clone();
    let cases = [
        (
            state_with(step.clone(), None, None, None),
            set_direction_act(commission, DirectionStatus::ChangesRequested),
        ),
        (
            state_with(step.clone(), None, None, None),
            set_deadline_act(commission, further_ahead()),
        ),
        (
            state_with(
                step.clone(),
                None,
                None,
                Some(DirectionStatus::ChangesRequested),
            ),
            clear_direction_act(commission),
        ),
        (
            state_with(step.clone(), Some(further_ahead()), None, None),
            clear_deadline_act(commission),
        ),
        (
            state_with(step.clone(), Some(ahead()), None, None),
            set_delayed_act(commission),
        ),
    ];
    for (stale, act) in cases {
        a_stale_field_act_fails(ports, actor, commission, stale, act).await;
    }
    field_call(
        ports,
        actor,
        commission,
        FieldCall::ClearDelayed,
        Utc::now(),
    )
    .await
    .expect("the flag comes down");
    let flagged = state_with(step, Some(ahead()), Some(DeadlineStatus::Delayed), None);
    a_stale_field_act_fails(
        ports,
        actor,
        commission,
        flagged,
        clear_delayed_act(commission),
    )
    .await;
}

/// Each stale dispute act on `commission`, `actor`'s, held at Active: a raise checked from Draft
/// and a clear checked as Disputed while the row isn't; then, the dispute raised for real, a
/// raise checked from Active and a clear checked back to Draft while the row is Disputed from
/// Active. Each matches no row and fails; the real clear lands it back at Active.
async fn each_stale_dispute_act_fails(ports: &Ports, actor: &UserId, commission: CommissionId) {
    assert_eq!(step_of(ports, commission).await, LifecycleStep::Active);
    let undisputed = [
        (
            state_at(LifecycleStep::Draft),
            raise_dispute_act(commission),
        ),
        (
            state_disputed_from(LifecycleStep::Active),
            clear_dispute_act(commission),
        ),
    ];
    for (stale, act) in undisputed {
        a_stale_field_act_fails(ports, actor, commission, stale, act).await;
    }
    field_call(
        ports,
        actor,
        commission,
        FieldCall::RaiseDispute,
        Utc::now(),
    )
    .await
    .expect("the dispute is raised");
    let disputed = [
        (
            state_at(LifecycleStep::Active),
            raise_dispute_act(commission),
        ),
        (
            state_disputed_from(LifecycleStep::Draft),
            clear_dispute_act(commission),
        ),
    ];
    for (stale, act) in disputed {
        a_stale_field_act_fails(ports, actor, commission, stale, act).await;
    }
    field_call(
        ports,
        actor,
        commission,
        FieldCall::ClearDispute,
        Utc::now(),
    )
    .await
    .expect("the dispute clears");
    assert_eq!(step_of(ports, commission).await, LifecycleStep::Active);
}

// A grant whose state is stale checks a change from a value the row no longer holds: the
// compare-and-set matches no row, so the act fails and nothing lands. So for the lifecycle
// move, the dispute's raise and clear, and for each field write: the direction status, the
// deadline and the Delayed flag.
#[tokio::test]
async fn a_stale_from_matches_no_row_and_fails_the_act() {
    let world = world().await;
    world
        .backend
        .seed_lifecycle_step(untitled(), LifecycleStep::Active);

    a_stale_move_fails(&world.ports, &alice(), untitled()).await;
    each_stale_dispute_act_fails(&world.ports, &alice(), untitled()).await;
    each_stale_field_act_fails(&world.ports, &alice(), untitled()).await;
}

#[tokio::test]
async fn a_stale_from_matches_no_row_and_fails_the_act_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, "did:plc:den-write-pg-stale").await;
    move_by(&ports, &owner, commission, LifecycleStep::Active)
        .await
        .expect("the owner moves it to Active");

    a_stale_move_fails(&ports, &owner, commission).await;
    each_stale_dispute_act_fails(&ports, &owner, commission).await;
    each_stale_field_act_fails(&ports, &owner, commission).await;
}

/// `actor`'s move of `commission` from Draft to Active through the move use case on `ports`, its
/// units recorded: one unit, committed, holding the new step and exactly one `lifecycle_moved`
/// entry.
async fn one_entry_in_the_move_s_unit(ports: &mut Ports, actor: &UserId, commission: CommissionId) {
    let before = kinds(ports, commission).await;
    let log = record_units(ports);

    move_through_the_use_case(ports, actor, commission, LifecycleStep::Active)
        .await
        .expect("the move lands");

    let after = kinds(ports, commission).await;
    let (kept, added) = after.split_at(before.len());
    assert_eq!(log.events(), ["begin", "commit"]);
    assert_eq!(kept, before.as_slice());
    assert_eq!(added, [ChangelogEntryKind::LifecycleMoved]);
    assert_eq!(step_of(ports, commission).await, LifecycleStep::Active);
}

/// Each field act but the move, in order, on an Active commission that holds no status,
/// deadline or flag, with the kind and the literal payload of the one entry each writes: the
/// dispute raised and cleared, then each direction, deadline and Delayed act.
fn field_acts_and_their_entries() -> [(FieldCall, ChangelogEntryKind, serde_json::Value); 9] {
    let waiting = DirectionStatus::WaitingForInput;
    [
        (
            FieldCall::RaiseDispute,
            ChangelogEntryKind::LifecycleMoved,
            serde_json::json!({ "from": "active", "to": "disputed" }),
        ),
        (
            FieldCall::ClearDispute,
            ChangelogEntryKind::LifecycleMoved,
            serde_json::json!({ "from": "disputed", "to": "active" }),
        ),
        (
            FieldCall::SetDirection(waiting),
            ChangelogEntryKind::StatusChanged,
            serde_json::json!({ "from": null, "to": "waiting_for_input" }),
        ),
        (
            FieldCall::ClearDirection,
            ChangelogEntryKind::StatusChanged,
            serde_json::json!({ "from": "waiting_for_input", "to": null }),
        ),
        (
            FieldCall::SetDeadline(ahead()),
            ChangelogEntryKind::DeadlineSet,
            serde_json::json!({ "from": null, "to": "2099-03-01T00:00:00Z" }),
        ),
        (
            FieldCall::SetDeadline(further_ahead()),
            ChangelogEntryKind::DeadlineExtended,
            serde_json::json!({ "from": "2099-03-01T00:00:00Z", "to": "2099-04-01T00:00:00Z" }),
        ),
        (
            FieldCall::SetDelayed,
            ChangelogEntryKind::Delayed,
            serde_json::json!({
                "from": null,
                "to": "delayed",
                "deadline": "2099-04-01T00:00:00Z",
            }),
        ),
        (
            FieldCall::ClearDelayed,
            ChangelogEntryKind::Delayed,
            serde_json::json!({
                "from": "delayed",
                "to": null,
                "deadline": "2099-04-01T00:00:00Z",
            }),
        ),
        (
            FieldCall::ClearDeadline,
            ChangelogEntryKind::DeadlineSet,
            serde_json::json!({ "from": "2099-04-01T00:00:00Z", "to": null }),
        ),
    ]
}

/// `actor`'s field acts on `commission` through their typed use cases on `ports`, its units
/// recorded: each one unit, committed, holding exactly one entry by `actor` with its literal
/// payload; and the fields as the last act left them.
async fn one_entry_in_each_field_act_s_unit(
    ports: &mut Ports,
    actor: &UserId,
    commission: CommissionId,
) {
    let log = record_units(ports);
    for (call, kind, payload) in field_acts_and_their_entries() {
        let before = entries(ports, commission).await.len();
        let units = log.events().len();

        field_call(ports, actor, commission, call, Utc::now())
            .await
            .unwrap_or_else(|error| panic!("{call:?} lands: {error:?}"));

        let stream = entries(ports, commission).await;
        let [entry] = &stream[before..] else {
            panic!("{call:?}: one entry, {} of them", stream.len() - before);
        };
        assert_eq!(log.events()[units..], ["begin", "commit"], "{call:?}");
        assert_eq!(entry.kind, kind, "{call:?}");
        assert_eq!(entry.payload, payload, "{call:?}");
        assert_eq!(entry.payload.to_string(), payload.to_string(), "{call:?}");
        assert_eq!(entry.actor_id.as_ref(), Some(actor), "{call:?}");
        assert_eq!(entry.note, None, "{call:?}");
    }
    let left = fields_of(ports, commission).await;
    assert_eq!(left.lifecycle_step(), &LifecycleStep::Active);
    assert_eq!(left.dispute(), None);
    assert_eq!(left.direction_status(), None);
    assert_eq!(left.deadline(), None);
    assert_eq!(left.delayed_flag(), None);
}

// A field act's write and its one entry land in the act's one unit: the lifecycle move, the
// dispute's raise and clear, then each direction, deadline and Delayed act.
#[tokio::test]
async fn each_field_act_writes_one_entry_in_its_unit() {
    let mut world = world().await;

    one_entry_in_the_move_s_unit(&mut world.ports, &bob(), untitled()).await;
    one_entry_in_each_field_act_s_unit(&mut world.ports, &bob(), untitled()).await;
}

#[tokio::test]
async fn each_field_act_writes_one_entry_in_its_unit_on_postgres() {
    let pg = pg_runtime().await;
    let mut ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, "did:plc:den-write-pg-field-entry").await;

    one_entry_in_the_move_s_unit(&mut ports, &owner, commission).await;
    one_entry_in_each_field_act_s_unit(&mut ports, &owner, commission).await;
}

/// Untitled's world with every changelog append refused from here on, and the database that
/// lands them, to hand back.
async fn refusing_world() -> (test_world::World, Arc<dyn Database>) {
    let mut world = world().await;
    let landing = world.ports.database.clone();
    world.ports.database = Arc::new(RefusingChangelogDatabase(landing.clone()));
    (world, landing)
}

/// Whether `call` by bob on Untitled in `world`, whose appends are refused, fails and leaves the
/// fields as they were; `label` names it in the answer when it doesn't.
async fn stays_unwritten(
    world: &test_world::World,
    call: FieldCall,
    label: &'static str,
) -> Option<&'static str> {
    let before = fields_of(&world.ports, untitled()).await;
    let called = field_call(&world.ports, &bob(), untitled(), call, Utc::now()).await;
    let after = fields_of(&world.ports, untitled()).await;
    (called.is_ok() || after != before).then_some(label)
}

// An act's entry can't be appended: the act's unit rolls back whole, so its field isn't
// written. So for the lifecycle move, which leaves the step at Draft; the dispute's raise, which
// leaves no dispute stored; and its clear, which leaves the commission Disputed.
#[tokio::test]
async fn a_refused_append_leaves_the_field_unwritten() {
    let mut landed = Vec::new();

    let (moving, _) = refusing_world().await;
    let moved =
        move_through_the_use_case(&moving.ports, &bob(), untitled(), LifecycleStep::Active).await;
    let unmoved = step_of(&moving.ports, untitled()).await == LifecycleStep::Draft;
    if moved.is_ok() || !unmoved {
        landed.push("the move");
    }
    let (raising, _) = refusing_world().await;
    landed.extend(stays_unwritten(&raising, FieldCall::RaiseDispute, "the raise").await);
    let (mut clearing, landing) = refusing_world().await;
    let refusing = clearing.ports.database.clone();
    clearing.ports.database = landing;
    field_call(
        &clearing.ports,
        &bob(),
        untitled(),
        FieldCall::RaiseDispute,
        Utc::now(),
    )
    .await
    .expect("the dispute is raised");
    clearing.ports.database = refusing;
    landed.extend(stays_unwritten(&clearing, FieldCall::ClearDispute, "the clear").await);

    assert_eq!(
        landed,
        Vec::<&str>::new(),
        "a refused append left these written"
    );
    let disputed = fields_of(&clearing.ports, untitled()).await;
    assert_eq!(disputed.lifecycle_step(), &LifecycleStep::Disputed);
}

/// `actor`'s dispute on `commission`, held at Draft, raised at `raised_at` and then cleared:
/// Disputed from Draft since then, then Draft again with no dispute stored.
async fn a_dispute_from_draft_clears_back(ports: &Ports, actor: &UserId, commission: CommissionId) {
    assert_eq!(step_of(ports, commission).await, LifecycleStep::Draft);
    let raised_at = Utc::now();

    field_call(ports, actor, commission, FieldCall::RaiseDispute, raised_at)
        .await
        .expect("the dispute is raised");

    let disputed = fields_of(ports, commission).await;
    let held = disputed
        .dispute()
        .cloned()
        .expect("a Disputed commission keeps its dispute");
    assert_eq!(disputed.lifecycle_step(), &LifecycleStep::Disputed);
    assert_eq!(held.left, LifecycleStep::Draft);
    let began_gap = (held.began - raised_at).num_microseconds().map(i64::abs);
    assert!(
        began_gap.is_some_and(|gap| gap < 1),
        "the dispute began when it was raised, to the store's microsecond: {held:?}"
    );

    field_call(
        ports,
        actor,
        commission,
        FieldCall::ClearDispute,
        Utc::now(),
    )
    .await
    .expect("the dispute clears");

    let cleared = fields_of(ports, commission).await;
    assert_eq!(cleared.lifecycle_step(), &LifecycleStep::Draft);
    assert_eq!(cleared.dispute(), None::<&StoredDispute>);
}

// A dispute raised on a Draft commission clears back to Draft: the step it left, stored beside
// when it began, and forgotten once cleared.
#[tokio::test]
async fn a_dispute_from_draft_clears_back_to_draft() {
    let world = world().await;

    a_dispute_from_draft_clears_back(&world.ports, &bob(), untitled()).await;
}

#[tokio::test]
async fn a_dispute_from_draft_clears_back_to_draft_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, "did:plc:den-write-pg-dispute-draft").await;

    a_dispute_from_draft_clears_back(&ports, &owner, commission).await;
}

/// The field acts that change nothing on a commission holding no status, deadline or flag:
/// each clear.
const CLEARS_OF_NOTHING: [FieldCall; 3] = [
    FieldCall::ClearDirection,
    FieldCall::ClearDeadline,
    FieldCall::ClearDelayed,
];

/// The field acts that set what [`CLEARS_OF_NOTHING`]'s commission then holds, and that
/// change nothing when run again.
fn sets_of_what_is_held() -> [FieldCall; 3] {
    [
        FieldCall::SetDirection(DirectionStatus::WaitingForInput),
        FieldCall::SetDeadline(ahead()),
        FieldCall::SetDelayed,
    ]
}

/// Runs each of `calls`, `actor`'s, on `commission`: each lands, and leaves the fields and the
/// entries as they were.
async fn each_changes_nothing(
    ports: &Ports,
    actor: &UserId,
    commission: CommissionId,
    calls: &[FieldCall],
) {
    for call in calls {
        let fields = fields_of(ports, commission).await;
        let before = kinds(ports, commission).await;

        field_call(ports, actor, commission, *call, Utc::now())
            .await
            .unwrap_or_else(|error| panic!("{call:?} lands: {error:?}"));

        assert_eq!(fields_of(ports, commission).await, fields, "{call:?}");
        assert_eq!(kinds(ports, commission).await, before, "{call:?}");
    }
}

/// Runs each of `calls`, `actor`'s, on `commission`, each landing.
async fn each_lands(ports: &Ports, actor: &UserId, commission: CommissionId, calls: &[FieldCall]) {
    for call in calls {
        field_call(ports, actor, commission, *call, Utc::now())
            .await
            .unwrap_or_else(|error| panic!("{call:?} lands: {error:?}"));
    }
}

// A move to the step the commission holds lands, writes nothing and records nothing; so does
// each clear of nothing held, and each set of the value held: the direction status, the
// deadline, the Delayed flag already raised.
#[tokio::test]
async fn an_unchanged_value_writes_no_entry() {
    let world = world().await;
    let before = kinds(&world.ports, untitled()).await;

    move_by(&world.ports, &bob(), untitled(), LifecycleStep::Draft)
        .await
        .expect("a move to the held step lands");

    assert_eq!(kinds(&world.ports, untitled()).await, before);
    assert_eq!(
        step_of(&world.ports, untitled()).await,
        LifecycleStep::Draft
    );
    each_changes_nothing(&world.ports, &bob(), untitled(), &CLEARS_OF_NOTHING).await;
    let sets = sets_of_what_is_held();
    each_lands(&world.ports, &bob(), untitled(), &sets).await;
    each_changes_nothing(&world.ports, &bob(), untitled(), &sets).await;
}

// On PostgreSQL, a move to the held step runs no lifecycle write and appends nothing; nor does
// a clear of nothing held, nor a set of the value held.
#[tokio::test]
async fn an_unchanged_value_writes_no_entry_on_postgres() {
    let statements = StatementLog::default();
    let pg = pg_runtime_recording(&statements).await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, "did:plc:den-write-pg-held-step").await;
    let mark = statements.mark();

    move_by(&ports, &owner, commission, LifecycleStep::Draft)
        .await
        .expect("a move to the held step lands");

    let written = field_writes_since(&statements, mark);
    assert_eq!(written, Vec::<String>::new());
    assert_eq!(
        kinds(&ports, commission).await,
        [ChangelogEntryKind::Created]
    );
    let clears = statements.mark();
    each_changes_nothing(&ports, &owner, commission, &CLEARS_OF_NOTHING).await;
    assert_eq!(
        field_writes_since(&statements, clears),
        Vec::<String>::new()
    );
    let sets = sets_of_what_is_held();
    each_lands(&ports, &owner, commission, &sets).await;
    let again = statements.mark();
    each_changes_nothing(&ports, &owner, commission, &sets).await;
    assert_eq!(field_writes_since(&statements, again), Vec::<String>::new());
}

/// The field writes and changelog appends `statements` saw since `mark`, by query name; the
/// reads between the acts are left out.
fn field_writes_since(statements: &StatementLog, mark: usize) -> Vec<String> {
    let writes = [
        "commission/set_lifecycle",
        "commission/set_direction_status",
        "commission/set_deadline",
        "commission/set_deadline_status",
        "changelog/append",
    ];
    statements
        .since(mark)
        .iter()
        .filter_map(|statement| statement.query_name())
        .filter(|name| writes.contains(name))
        .map(str::to_owned)
        .collect()
}

/// `actor` leaves `commission`'s Delayed flag dormant, raised with a deadline that is then
/// cleared, and takes it down: the clear sees no status yet compares against the stored flag,
/// so it lands and records `from`, `to` and `deadline` all null.
async fn a_dormant_flag_s_clear_lands(ports: &Ports, actor: &UserId, commission: CommissionId) {
    let raise = [
        FieldCall::SetDeadline(ahead()),
        FieldCall::SetDelayed,
        FieldCall::ClearDeadline,
    ];
    each_lands(ports, actor, commission, &raise).await;
    let dormant = fields_of(ports, commission).await;
    assert_eq!(dormant.deadline(), None);
    assert_eq!(dormant.delayed_flag(), Some(DeadlineStatus::Delayed));
    let before = entries(ports, commission).await.len();

    field_call(
        ports,
        actor,
        commission,
        FieldCall::ClearDelayed,
        Utc::now(),
    )
    .await
    .expect("the dormant flag's clear lands");

    let stream = entries(ports, commission).await;
    let [entry] = &stream[before..] else {
        panic!("one entry: {} of them", stream.len() - before);
    };
    let payload = serde_json::json!({ "from": null, "to": null, "deadline": null });
    assert_eq!(entry.kind, ChangelogEntryKind::Delayed);
    assert_eq!(entry.payload, payload);
    assert_eq!(fields_of(ports, commission).await.delayed_flag(), None);
}

// A Delayed flag left after its deadline was cleared is taken down: the compare-and-set reads
// the stored flag, not the status the act saw.
#[tokio::test]
async fn a_dormant_flag_s_clear_lands_and_records_no_status() {
    let world = world().await;

    a_dormant_flag_s_clear_lands(&world.ports, &bob(), untitled()).await;
}

#[tokio::test]
async fn a_dormant_flag_s_clear_lands_and_records_no_status_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, commission) = pg_seed(&ports, "did:plc:den-write-pg-dormant").await;

    a_dormant_flag_s_clear_lands(&ports, &owner, commission).await;
}

/// A deadline before 1970, and so before 2000, where the store counts its microseconds from.
fn before_the_epoch() -> chrono::DateTime<Utc> {
    "1969-07-20T20:17:40.123456Z".parse().expect("a timestamp")
}

/// `instant` plus `nanos` nanoseconds: finer than the store holds.
fn plus_nanos(instant: chrono::DateTime<Utc>, nanos: i64) -> chrono::DateTime<Utc> {
    instant + chrono::Duration::nanoseconds(nanos)
}

/// The deadline kinds of `commission`'s entries, in stream order.
async fn deadline_kinds(ports: &Ports, commission: CommissionId) -> Vec<ChangelogEntryKind> {
    let deadline_kinds = [
        ChangelogEntryKind::DeadlineSet,
        ChangelogEntryKind::DeadlineExtended,
    ];
    let all = kinds(ports, commission).await;
    all.into_iter()
        .filter(|kind| deadline_kinds.contains(kind))
        .collect()
}

/// A commission of `owner`'s with `deadline`, made by the create use case.
async fn made_with(
    ports: &Ports,
    owner: &UserId,
    deadline: Option<chrono::DateTime<Utc>>,
) -> CommissionId {
    let command = create::Command {
        actor_id: owner.clone(),
        title: "A dated piece".parse().expect("a title"),
        maturity: None,
        deadline,
    };
    let created = Commissions::new(ports)
        .create(command, Utc::now())
        .await
        .expect("the commission is made");
    created.id
}

/// `actor` sets `commission`'s deadline to `held`, then to `held` plus one and two nanoseconds:
/// each finer set lands, changes no field and records nothing; the deadline reads back as
/// `held`, and the one deadline entry is the first set's.
async fn a_finer_set_changes_nothing(
    ports: &Ports,
    actor: &UserId,
    commission: CommissionId,
    held: chrono::DateTime<Utc>,
) {
    each_lands(ports, actor, commission, &[FieldCall::SetDeadline(held)]).await;
    let finer = [
        FieldCall::SetDeadline(plus_nanos(held, 1)),
        FieldCall::SetDeadline(plus_nanos(held, 2)),
    ];

    each_changes_nothing(ports, actor, commission, &finer).await;

    assert_eq!(fields_of(ports, commission).await.deadline(), Some(held));
    assert_eq!(
        deadline_kinds(ports, commission).await,
        [ChangelogEntryKind::DeadlineSet]
    );
}

// A deadline set finer than the store holds is the deadline the store would keep: set D, then
// D + 1 ns and D + 2 ns land, write nothing and record nothing, after 2000 and before 1970.
#[tokio::test]
async fn a_sub_microsecond_move_writes_no_entry() {
    let world = world().await;
    let ports = &world.ports;

    for held in [ahead(), before_the_epoch()] {
        let commission = made_with(ports, &alice(), None).await;
        a_finer_set_changes_nothing(ports, &alice(), commission, held).await;
    }
}

// On PostgreSQL, the finer sets run no deadline write and append nothing, and the deadline
// reads back as the one first set.
#[tokio::test]
async fn a_sub_microsecond_move_writes_no_entry_on_postgres() {
    let statements = StatementLog::default();
    let pg = pg_runtime_recording(&statements).await;
    let ports = Ports::from(&pg.runtime);
    let owner = provision(&*ports.database, "did:plc:den-write-pg-finer").await;

    for held in [ahead(), before_the_epoch()] {
        let commission = made_with(&ports, &owner, None).await;
        let mark = statements.mark();
        a_finer_set_changes_nothing(&ports, &owner, commission, held).await;
        let written = field_writes_since(&statements, mark);
        assert_eq!(written, ["commission/set_deadline", "changelog/append"]);
    }
}

/// A commission of `owner`'s made with a deadline one nanosecond past [`before_the_epoch`]
/// holds [`before_the_epoch`] itself, so a set of it changes nothing.
async fn a_create_holds_the_stored_deadline(ports: &Ports, owner: &UserId) {
    let held = before_the_epoch();
    let commission = made_with(ports, owner, Some(plus_nanos(held, 1))).await;

    assert_eq!(fields_of(ports, commission).await.deadline(), Some(held));
    each_changes_nothing(ports, owner, commission, &[FieldCall::SetDeadline(held)]).await;
    assert_eq!(
        deadline_kinds(ports, commission).await,
        Vec::<ChangelogEntryKind>::new()
    );
}

// A create's deadline is held as the store would keep it, so both stores hold the same one.
#[tokio::test]
async fn a_create_holds_its_deadline_at_the_store_s_precision() {
    let world = world().await;

    a_create_holds_the_stored_deadline(&world.ports, &alice()).await;
}

#[tokio::test]
async fn a_create_holds_its_deadline_at_the_store_s_precision_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let owner = provision(&*ports.database, "did:plc:den-write-pg-create-precision").await;

    a_create_holds_the_stored_deadline(&ports, &owner).await;
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

/// The title [`pg_seed`] gives its commission.
pub(super) const PG_TITLE: &str = "Seeded piece";

/// The owner `owner_did`, recognized, and a commission of theirs titled [`PG_TITLE`], made by
/// the create use case.
pub(super) async fn pg_seed(ports: &Ports, owner_did: &str) -> (UserId, CommissionId) {
    let owner = provision(&*ports.database, owner_did).await;
    let command = create::Command {
        actor_id: owner.clone(),
        title: PG_TITLE.parse().expect("a title"),
        maturity: None,
        deadline: None,
    };
    let created = Commissions::new(ports)
        .create(command, Utc::now())
        .await
        .expect("the commission seeds");
    (owner, created.id)
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
