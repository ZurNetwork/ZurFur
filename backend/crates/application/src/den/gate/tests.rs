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
    elements::{
        commission::{ChangelogEntry, ChangelogEntryKind},
        did::Did,
        level::Level,
        user::UserId,
    },
    ports::{Database, UnitOfWork},
};
use test_support::pg_app::{StatementLog, pg_runtime, pg_runtime_recording};

use super::*;
use crate::{
    Ports,
    den::{
        Den, DenThing,
        act::{EntryConstructor, GatedEntry},
        authority::{GrantKind, MINT_AS},
        test_acts::{
            commission_path, create_act, hand_built_create, hand_built_rename, rename_act,
        },
        write::{Command, DenWriteError, WriteResult},
    },
    test_world, transaction,
};
use spy::{Spy, SpyDatabase};

/// The title every create here gives its commission, which no entry may carry.
const TITLE: &str = "A gated piece";

/// The title a rename here gives, which no entry may carry either.
const NEW_TITLE: &str = "A renamed piece";

/// A grant on "Untitled" to `actor` in `role`, bound to `node`.
fn existing_on(actor: &UserId, role: CommissionRole, node: NodeId) -> Grant {
    let kind = GrantKind::Existing {
        commission: untitled(),
        role,
        level: Level::Private,
        node,
    };
    Grant::for_test(actor, kind)
}

/// A grant on "Untitled" to `actor` in `role`, bound to a node no row holds.
fn existing(actor: &UserId, role: CommissionRole) -> Grant {
    existing_on(actor, role, NodeId::mint())
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
    let act = hand_built_rename(commission_path(untitled()), untitled(), "Taken over");
    let node = top_row(database, untitled()).await.expect("seeded").id;

    let applied = through_host(
        database,
        DenHost::viewer_of(&bob()),
        existing_on(&alice(), CommissionRole::Owner, node),
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
    let act = hand_built_create(minted, "Made here");

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

/// A creator that drops a failure it got through its lent view, then answers as if none came.
#[derive(Debug, Clone, Copy)]
enum Swallowing {
    /// Reads the commission before it exists and drops that miss, then creates it.
    Miss,
    /// Creates the commission and reads it, then creates it again and drops that failure.
    StoreFailure,
}

impl Handler for Swallowing {
    type Role = Creator;

    const ENTRY: EntryRule = EntryRule::Always(GatedEntry::CREATED);

    async fn handle(&self, lent: &mut Lent<'_, Creator>) -> Result<Handled, HandlerError> {
        let chosen = NewCommission {
            maturity: None,
            deadline: None,
        };
        let answer = match self {
            Self::Miss => {
                let _dropped = lent.lent().await;
                lent.create(&chosen).await?;
                lent.lent().await?
            }
            Self::StoreFailure => {
                lent.create(&chosen).await?;
                let read = lent.lent().await?;
                let _dropped = lent.create(&chosen).await;
                read
            }
        };
        Ok(Handled { answer })
    }
}

/// Each swallowing creator, lent a create grant through [`lend_as`] on a unit of `database`
/// holding its commission's node, fails as a store failure; the unit then keeps neither its
/// node nor its commission.
async fn each_swallowed_failure_fails_the_act(ports: &Ports, actor: &UserId) {
    let database = &*ports.database;
    for handler in [Swallowing::Miss, Swallowing::StoreFailure] {
        let planted = CommissionId::mint();
        let segment = NodeSegment::try_from(planted.to_string()).expect("an id is a segment");
        let node = NewNode {
            id: NodeId::mint(),
            parent: None,
            segment,
            node_type: NodeType::Commission,
            level: Level::Private,
            name: StoredText::from(TITLE.to_owned()),
        };
        let written = Written {
            node: node.id,
            commission: planted,
            name: None,
            level: atta::Level::Private,
            changed: true,
        };
        let grant = creating(actor, planted);
        let mut unit = database.begin().await.expect("begin");
        unit.nodes()
            .create(&node)
            .await
            .expect("the node row is made");

        let landed = lend_as(&handler, &grant, &mut *unit, &written, Utc::now()).await;
        unit.rollback().await.expect("the unit ends");

        assert!(landed.is_err(), "{handler:?} landed");
        assert!(top_row(database, planted).await.is_none(), "{handler:?}");
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

// The rename's handler reads its commission through the lent view; the read fails, so the act
// fails as a store failure and the unit keeps neither the new name nor an entry.
#[tokio::test]
async fn a_failed_handler_writes_neither_row_nor_entry() {
    let spy = Spy::failing_reads();
    let world = spied(&spy).await;
    let before = entries(&world.ports, untitled()).await.len();

    let written = write_at(
        &world.ports,
        &alice(),
        rename_act(untitled(), NEW_TITLE),
        Utc::now(),
    )
    .await;

    assert!(
        matches!(written, Err(DenWriteError::Store(_))),
        "{written:?}"
    );
    let row = top_row(&*world.ports.database, untitled())
        .await
        .expect("seeded");
    assert_eq!(row.name.as_str(), "Untitled");
    assert_eq!(entries(&world.ports, untitled()).await.len(), before);
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

/// A create and then a rename by `actor` on `ports` write their entries as the gate builds
/// them: on the made commission, by `actor`, with an empty payload, no note and no title
/// anywhere.
async fn the_entry_is_the_gate_s(ports: &Ports, actor: &UserId) {
    let written = write_at(ports, actor, create_act(TITLE), Utc::now())
        .await
        .expect("the create lands");
    let minted = made(&written);
    write_at(ports, actor, rename_act(minted, NEW_TITLE), Utc::now())
        .await
        .expect("the rename lands");

    let stream = entries(ports, minted).await;
    let [created, renamed] = stream.as_slice() else {
        panic!("two entries: {} of them", stream.len());
    };
    let empty_payload = serde_json::json!({});
    let expected = [
        (created, ChangelogEntryKind::Created),
        (renamed, ChangelogEntryKind::Renamed),
    ];
    for (entry, kind) in expected {
        assert_eq!(entry.kind, kind);
        assert_eq!(entry.commission_id, minted, "{kind}");
        assert_eq!(entry.actor_id.as_ref(), Some(actor), "{kind}");
        assert_eq!(entry.payload, empty_payload, "{kind}");
        assert_eq!(entry.note, None, "{kind}");
        let payload = entry.payload.to_string();
        assert!(!payload.contains(TITLE), "{kind}");
        assert!(!payload.contains(NEW_TITLE), "{kind}");
    }
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

/// A constructor that builds a create's entry on another commission than it was given.
fn on_another_commission(_: CommissionId, actor: UserId, at: DateTimeUtc) -> NewChangelogEntry {
    NewChangelogEntry::created(old_sketch(), actor, at)
}

/// A constructor that builds a create's entry by another actor than it was given.
fn by_another_actor(commission: CommissionId, _: UserId, at: DateTimeUtc) -> NewChangelogEntry {
    NewChangelogEntry::created(commission, bob(), at)
}

/// A constructor that builds a create's entry at another time than it was given.
fn at_another_time(commission: CommissionId, actor: UserId, at: DateTimeUtc) -> NewChangelogEntry {
    let earlier = at - chrono::TimeDelta::seconds(1);
    NewChangelogEntry::created(commission, actor, earlier)
}

/// What the gate wrote for an act that changed its node on "Untitled".
fn changed_untitled() -> Written {
    Written {
        node: NodeId::mint(),
        commission: untitled(),
        name: None,
        level: atta::Level::Private,
        changed: true,
    }
}

// A gated entry's constructor building off the grant's commission, actor or time is a broken
// rule, and the gate appends nothing.
#[test]
fn an_entry_built_off_the_grant_is_a_broken_rule() {
    let now = Utc::now();
    let written = changed_untitled();
    let misbuilt: [EntryConstructor; 3] =
        [on_another_commission, by_another_actor, at_another_time];

    for constructor in misbuilt {
        let gated = GatedEntry::for_test(ChangelogEntryKind::Created, constructor);
        let built = built_entry(EntryRule::Always(gated), &written, &alice(), now);
        assert!(built.is_err(), "{built:?}");
    }
    let built = built_entry(
        EntryRule::Always(GatedEntry::CREATED),
        &written,
        &alice(),
        now,
    )
    .expect("the domain's constructor builds as granted");
    assert!(built.is_some());
}

/// A constructor that builds an archive's entry carrying a title and a note, whatever kind its
/// gated entry names.
fn leaky(commission: CommissionId, actor: UserId, at: DateTimeUtc) -> NewChangelogEntry {
    let payload = serde_json::json!({ "title": "A secret sketch" });
    NewChangelogEntry::event(commission, ChangelogEntryKind::Archived, actor, payload, at)
        .with_note("free text the act chose".to_owned())
}

// A gated entry whose constructor builds another kind than its own is a broken rule.
#[test]
fn an_entry_of_another_kind_than_its_gated_entry_s_is_a_broken_rule() {
    let gated = GatedEntry::for_test(ChangelogEntryKind::Renamed, leaky);

    let built = built_entry(
        EntryRule::OnChange(gated),
        &changed_untitled(),
        &alice(),
        Utc::now(),
    );

    assert!(built.is_err(), "{built:?}");
}

/// The leaky constructor under kind `Renamed`, appended through the gate's own append on a unit
/// of `ports` that commits whatever the append answers: refused as a broken rule, and the
/// commission's changelog is as it was.
async fn another_kind_appends_nothing(ports: &Ports, actor: &UserId, commission: CommissionId) {
    let before = entries(ports, commission).await.len();
    let gated = GatedEntry::for_test(ChangelogEntryKind::Renamed, leaky);
    let written = Written {
        commission,
        ..changed_untitled()
    };
    let mut unit = ports.database.begin().await.expect("begin");

    let appended = append_entry(
        &mut *unit,
        EntryRule::OnChange(gated),
        &written,
        actor,
        Utc::now(),
    )
    .await;
    unit.commit().await.expect("the unit commits what it holds");

    assert!(appended.is_err(), "{appended:?}");
    assert_eq!(entries(ports, commission).await.len(), before);
}

// The leaky constructor's archive entry, with a title in its payload and a note, is refused
// under kind `Renamed`, and nothing is appended.
#[tokio::test]
async fn an_entry_of_another_kind_than_its_gated_entry_s_appends_nothing() {
    let world = test_world::world().await;

    another_kind_appends_nothing(&world.ports, &alice(), untitled()).await;
}

#[tokio::test]
async fn an_entry_of_another_kind_than_its_gated_entry_s_appends_nothing_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let actor = provision(&*ports.database, "did:plc:gate-another-kind").await;
    let written = write_at(&ports, &actor, create_act(TITLE), Utc::now())
        .await
        .expect("the create lands");

    another_kind_appends_nothing(&ports, &actor, made(&written)).await;
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

/// The node statements PostgreSQL ran since `mark`, by query name.
fn node_statements(statements: &StatementLog, mark: usize) -> Vec<&'static str> {
    statements
        .since(mark)
        .iter()
        .filter_map(|statement| statement.query_name())
        .filter(|name| name.starts_with("node/"))
        .collect()
}

// On PostgreSQL, a create writes its seven node rows and reads none, and a rename reads its
// node once on the walk, writes its name, and reads none after: each answer comes from what
// the gate wrote and the handler's commission read.
#[tokio::test]
async fn create_and_rename_read_no_node_after_writing() {
    let statements = StatementLog::default();
    let pg = pg_runtime_recording(&statements).await;
    let ports = Ports::from(&pg.runtime);
    let actor = provision(&*ports.database, "did:plc:gate-no-node-read").await;
    let before_create = statements.mark();

    let written = write_at(&ports, &actor, create_act(TITLE), Utc::now())
        .await
        .expect("the create lands");
    let created = node_statements(&statements, before_create);
    let before_rename = statements.mark();
    write_at(
        &ports,
        &actor,
        rename_act(made(&written), NEW_TITLE),
        Utc::now(),
    )
    .await
    .expect("the rename lands");
    let renamed = node_statements(&statements, before_rename);

    assert_eq!(created, ["node/create"; 7]);
    assert_eq!(renamed, ["node/at_top", "node/set_name"]);
}
