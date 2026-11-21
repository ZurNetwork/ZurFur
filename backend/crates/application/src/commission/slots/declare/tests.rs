#![allow(
    clippy::disallowed_methods,
    reason = "a test reads the node rows back on a unit of its own"
)]

use std::sync::Arc;

use adapter_mem::fixture::{alice, bob, old_sketch, untitled};
use chrono::Utc;
use domain::{
    elements::{
        commission::{ChangelogEntry, ChangelogEntryKind, TabId},
        did::Did,
        level::Level,
        node::{NodeRow, NodeSegment, NodeType},
    },
    ports::{Database, UnitOfWork},
};
use test_support::{
    pg_app::pg_runtime,
    recording::{CallLog, RecordingDatabase},
};

use super::*;
use crate::{
    commission::{Commissions, create},
    common_error::{CommonError, NotFoundEntity},
    test_world, transaction,
};

/// A title no log, entry or answer but the Slot's own may carry.
const CANARY_TITLE: &str = "Canary knight 4f1e";

/// Notes no log, entry or answer but the Slot's own may carry.
const CANARY_NOTES: &str = "Canary notes 8b2d";

/// `title` as a Slot title.
fn title(text: &str) -> SlotTitle {
    text.parse().expect("a valid slot title")
}

/// One Slot asked for on `tab`'s `content` surface, titled `text`, with `notes`.
fn body(tab: TabId, text: &str, notes: Option<&str>) -> SlotBody {
    let surface = SurfaceAddress::new(tab, "content".parse().expect("a surface name"));
    SlotBody {
        surface,
        title: title(text),
        notes: notes.map(str::to_owned),
    }
}

/// `actor`'s declare of `slots` on `commission`, through the use case.
async fn declare_by(
    ports: &Ports,
    actor: &UserId,
    commission: CommissionId,
    slots: Vec<SlotBody>,
) -> CommissionResult<Output> {
    let command = Command {
        user_id: actor.clone(),
        commission_id: commission,
        slots,
    };
    Commissions::new(ports)
        .slots()
        .declare(command, Utc::now())
        .await
}

/// The first tab of `commission`, read from its committed composition.
async fn tab_of(ports: &Ports, commission: CommissionId) -> TabId {
    let composition = ports
        .commissions
        .load_composition(&commission)
        .await
        .expect("the composition reads")
        .expect("the commission composes");
    composition
        .tabs
        .first()
        .expect("the commission has its tab")
        .id
}

/// Every changelog entry of `commission`.
async fn entries(ports: &Ports, commission: CommissionId) -> Vec<ChangelogEntry> {
    ports
        .changelog
        .entries(&commission)
        .await
        .expect("the changelog reads")
}

/// The node rows of `commission`'s `slots` entry and of every Slot node under it, read back on
/// a unit that keeps nothing.
async fn slots_rows(database: &dyn Database, commission: CommissionId) -> (NodeRow, Vec<NodeRow>) {
    let mut unit = database.begin().await.expect("begin");
    let top = NodeSegment::try_from(commission.to_string()).expect("an id is a segment");
    let commission_row = unit
        .nodes()
        .at_top(&NodeType::Commission, &top)
        .await
        .expect("the node rows read")
        .expect("the commission has its node");
    let word = NodeSegment::try_from("slots").expect("a word is a segment");
    let entry = unit
        .nodes()
        .child(&commission_row.id, &word)
        .await
        .expect("the node rows read")
        .expect("the commission has its slots entry");
    let slots = unit
        .nodes()
        .children(&entry.id)
        .await
        .expect("the node rows read");
    unit.rollback().await.expect("the read's unit ends");
    (entry, slots)
}

/// Records `begin`, `commit` and `rollback` on `ports`' database from here on.
fn record_units(ports: &mut Ports) -> CallLog {
    let log = CallLog::default();
    let recording = RecordingDatabase::new(ports.database.clone(), log.clone());
    ports.database = Arc::new(recording);
    log
}

/// A recognized owner and a commission of theirs, made by the create use case, on `ports`.
async fn seed_owned(ports: &Ports, owner_did: &str) -> (UserId, CommissionId) {
    let did = Did::from(owner_did.to_owned());
    let owner = transaction(&*ports.database, async move |uow: &mut dyn UnitOfWork| {
        let user = uow.users().provision(&did).await?;
        Ok(user.id)
    })
    .await
    .expect("the owner is recognized");
    let command = create::Command {
        actor_id: owner.clone(),
        title: "A piece with Slots".parse().expect("a title"),
        maturity: None,
        deadline: None,
    };
    let created = Commissions::new(ports)
        .create(command, Utc::now())
        .await
        .expect("the commission is created");
    (owner, created.id)
}

/// The declared Slot points at the node the gate made: one Private `slot` node under the
/// commission's `slots` entry, keyed by the Slot's element id and named by its title, and the
/// Slot read titled by that node, on `ports`.
async fn assert_the_slot_points_at_its_node(ports: &Ports, owner: &UserId, id: CommissionId) {
    let tab = tab_of(ports, id).await;
    let (_, before) = slots_rows(&*ports.database, id).await;

    let declared = declare_by(ports, owner, id, vec![body(tab, CANARY_TITLE, None)])
        .await
        .expect("the owner declares a Slot");

    let [slot_id] = declared.slot_ids.as_slice() else {
        panic!("one Slot was declared");
    };
    let (entry, after) = slots_rows(&*ports.database, id).await;
    let made: Vec<&NodeRow> = after
        .iter()
        .filter(|row| !before.iter().any(|old| old.id == row.id))
        .collect();
    let [node] = made.as_slice() else {
        panic!("one node was made: {made:?}");
    };
    assert_eq!(node.node_type, NodeType::Slot);
    assert_eq!(node.parent, Some(entry.id));
    assert_eq!(node.segment.as_ref(), slot_id.to_string());
    assert_eq!(node.level, Level::Private);
    assert_eq!(node.name.as_str(), CANARY_TITLE);
    let listed = ports.commissions.slots(&id).await.expect("the Slots read");
    let read = listed
        .iter()
        .find(|slot| slot.element_id == *slot_id)
        .expect("the Slot is listed");
    assert_eq!(read.title.as_str(), node.name.as_str());
}

// On memory, the declared Slot points at the node the gate made for it.
#[tokio::test]
async fn the_declared_slot_points_at_the_node_the_gate_made() {
    let world = test_world::world().await;

    assert_the_slot_points_at_its_node(&world.ports, &alice(), untitled()).await;
}

// On PostgreSQL, the declared Slot points at the node the gate made for it.
#[tokio::test]
async fn the_declared_slot_points_at_the_node_the_gate_made_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, id) = seed_owned(&ports, "did:plc:declare-pg-node").await;

    assert_the_slot_points_at_its_node(&ports, &owner, id).await;
}

// A declare makes only `slot` nodes, one per Slot, each under the commission's `slots` entry;
// no other node is made or changed.
#[tokio::test]
async fn a_declare_makes_only_slot_nodes_under_slots() {
    let world = test_world::world().await;
    let tab = tab_of(&world.ports, untitled()).await;
    let (entry, _) = slots_rows(&*world.ports.database, untitled()).await;
    let before = world.backend.all_nodes();
    let slots = vec![
        body(tab, "First", None),
        body(tab, "Second", None),
        body(tab, "Third", Some("notes")),
    ];

    let declared = declare_by(&world.ports, &alice(), untitled(), slots)
        .await
        .expect("the owner declares three Slots");

    let after = world.backend.all_nodes();
    let made: Vec<&NodeRow> = after.iter().filter(|row| !before.contains(row)).collect();
    let kept = before.iter().all(|row| after.contains(row));
    assert!(kept, "no node is removed or changed");
    assert_eq!(made.len(), 3, "{made:?}");
    for row in made {
        assert_eq!(row.node_type, NodeType::Slot, "{row:?}");
        assert_eq!(row.parent, Some(entry.id), "{row:?}");
        let keyed_by_a_slot = declared
            .slot_ids
            .iter()
            .any(|slot| row.segment.as_ref() == slot.to_string());
        assert!(keyed_by_a_slot, "{row:?}");
    }
}

// A `slots` entry wider than the Private its act takes it as fails the declare closed: the
// walked row and the act disagree, so nothing is written.
#[tokio::test]
async fn a_slots_entry_wider_than_its_placed_level_fails_closed() {
    let world = test_world::world().await;
    let tab = tab_of(&world.ports, untitled()).await;
    let (entry, before) = slots_rows(&*world.ports.database, untitled()).await;
    world.backend.seed_node_level(entry.id, Level::Listed);
    let entries_before = entries(&world.ports, untitled()).await.len();

    let declared = declare_by(
        &world.ports,
        &alice(),
        untitled(),
        vec![body(tab, "Never", None)],
    )
    .await;

    assert!(
        matches!(
            declared,
            Err(CommissionError::CommonError(CommonError::Infrastructure(_)))
        ),
        "{declared:?}"
    );
    let (_, after) = slots_rows(&*world.ports.database, untitled()).await;
    assert_eq!(after, before, "no Slot node is made");
    let entries_after = entries(&world.ports, untitled()).await.len();
    assert_eq!(entries_after, entries_before, "no entry is written");
}

// One Slot on a tab the commission doesn't hold fails the whole declare: the Slot before it is
// not declared, and no node, element or entry is written.
#[tokio::test]
async fn a_declare_with_one_bad_tab_declares_none() {
    let world = test_world::world().await;
    let tab = tab_of(&world.ports, untitled()).await;
    let (_, nodes_before) = slots_rows(&*world.ports.database, untitled()).await;
    let slots_before = world.backend.slots_of(untitled()).await.expect("slots");
    let elements_before = world
        .backend
        .elements_of(untitled())
        .await
        .expect("elements");
    let entries_before = entries(&world.ports, untitled()).await.len();
    let slots = vec![
        body(tab, "Would be fine alone", None),
        body(TabId::mint(), "Doomed", None),
    ];

    let declared = declare_by(&world.ports, &alice(), untitled(), slots).await;

    assert!(
        matches!(declared, Err(CommissionError::TabNotFound)),
        "{declared:?}"
    );
    let (_, nodes_after) = slots_rows(&*world.ports.database, untitled()).await;
    assert_eq!(nodes_after, nodes_before);
    let slots_after = world.backend.slots_of(untitled()).await.expect("slots");
    assert_eq!(slots_after, slots_before);
    let elements_after = world
        .backend
        .elements_of(untitled())
        .await
        .expect("elements");
    assert_eq!(elements_after.len(), elements_before.len());
    let entries_after = entries(&world.ports, untitled()).await.len();
    assert_eq!(entries_after, entries_before);
}

// Another commission's tab and a tab nobody holds answer alike, and neither writes anything.
#[tokio::test]
async fn a_foreign_tab_and_a_missing_one_answer_alike() {
    let world = test_world::world().await;
    let foreign = tab_of(&world.ports, old_sketch()).await;
    let (_, nodes_before) = slots_rows(&*world.ports.database, untitled()).await;

    let through_foreign = declare_by(
        &world.ports,
        &alice(),
        untitled(),
        vec![body(foreign, "Foreign", None)],
    )
    .await;
    let through_missing = declare_by(
        &world.ports,
        &alice(),
        untitled(),
        vec![body(TabId::mint(), "Missing", None)],
    )
    .await;

    assert!(
        matches!(through_foreign, Err(CommissionError::TabNotFound)),
        "{through_foreign:?}"
    );
    assert_eq!(
        format!("{through_foreign:?}"),
        format!("{through_missing:?}")
    );
    let (_, nodes_after) = slots_rows(&*world.ports.database, untitled()).await;
    assert_eq!(nodes_after, nodes_before);
}

/// Each declared Slot writes one `slot_declared` entry by the owner, in declare order, its
/// payload exactly `{"slot":"<element id>"}` and no note, on `ports`.
async fn assert_one_untitled_entry_per_slot(ports: &Ports, owner: &UserId, id: CommissionId) {
    let tab = tab_of(ports, id).await;
    let before = entries(ports, id).await.len();
    let slots = vec![
        body(tab, CANARY_TITLE, Some(CANARY_NOTES)),
        body(tab, "Second", None),
    ];

    let declared = declare_by(ports, owner, id, slots)
        .await
        .expect("the owner declares two Slots");

    let stream = entries(ports, id).await;
    let written = &stream[before..];
    assert_eq!(written.len(), declared.slot_ids.len());
    for (entry, slot) in written.iter().zip(&declared.slot_ids) {
        let literal = format!("{{\"slot\":\"{slot}\"}}");
        assert_eq!(entry.kind, ChangelogEntryKind::SlotDeclared);
        assert_eq!(entry.actor_id.as_ref(), Some(owner));
        assert_eq!(entry.payload.to_string(), literal);
        assert_eq!(entry.note, None);
        let printed = format!("{entry:?}");
        assert!(!printed.contains(CANARY_TITLE), "{printed}");
        assert!(!printed.contains(CANARY_NOTES), "{printed}");
    }
}

// On memory, each declared Slot writes one entry naming it alone.
#[tokio::test]
async fn each_declared_slot_writes_one_untitled_entry() {
    let world = test_world::world().await;

    assert_one_untitled_entry_per_slot(&world.ports, &alice(), untitled()).await;
}

// On PostgreSQL, each declared Slot writes one entry naming it alone.
#[tokio::test]
async fn each_declared_slot_writes_one_untitled_entry_on_postgres() {
    let pg = pg_runtime().await;
    let ports = Ports::from(&pg.runtime);
    let (owner, id) = seed_owned(&ports, "did:plc:declare-pg-entries").await;

    assert_one_untitled_entry_per_slot(&ports, &owner, id).await;
}

// A Participant who isn't the owner may not declare: forbidden, and nothing is written.
#[tokio::test]
async fn a_participant_s_declare_is_forbidden() {
    let world = test_world::world().await;
    let tab = tab_of(&world.ports, untitled()).await;
    let (_, nodes_before) = slots_rows(&*world.ports.database, untitled()).await;
    let entries_before = entries(&world.ports, untitled()).await.len();

    let declared = declare_by(
        &world.ports,
        &bob(),
        untitled(),
        vec![body(tab, "Bob's", None)],
    )
    .await;

    assert!(
        matches!(declared, Err(CommissionError::InsufficientPermissions)),
        "{declared:?}"
    );
    let (_, nodes_after) = slots_rows(&*world.ports.database, untitled()).await;
    assert_eq!(nodes_after, nodes_before);
    assert_eq!(
        entries(&world.ports, untitled()).await.len(),
        entries_before
    );
}

// A stranger's declare and one on a made-up id get the same not-found, and neither opens a
// unit.
#[tokio::test]
async fn a_stranger_and_a_made_up_id_get_the_same_not_found() {
    let mut world = test_world::world().await;
    let log = record_units(&mut world.ports);
    let tab = tab_of(&world.ports, untitled()).await;
    let stranger = test_world::stranger();

    let not_yours = declare_by(
        &world.ports,
        &stranger,
        untitled(),
        vec![body(tab, "Probe", None)],
    )
    .await;
    let absent = declare_by(
        &world.ports,
        &stranger,
        test_world::absent_commission(),
        vec![body(tab, "Probe", None)],
    )
    .await;

    assert!(
        matches!(
            not_yours,
            Err(CommissionError::CommonError(CommonError::NotFound(
                NotFoundEntity::Commission
            )))
        ),
        "{not_yours:?}"
    );
    assert_eq!(format!("{not_yours:?}"), format!("{absent:?}"));
    assert!(
        log.events().is_empty(),
        "no unit opened: {:?}",
        log.events()
    );
}

// No Slot asked for is refused as empty content, before anyone's standing.
#[tokio::test]
async fn a_declare_of_no_slot_is_refused() {
    let mut world = test_world::world().await;
    let log = record_units(&mut world.ports);

    let declared = declare_by(&world.ports, &alice(), untitled(), Vec::new()).await;

    assert!(
        matches!(declared, Err(CommissionError::IncorrectContent)),
        "{declared:?}"
    );
    assert!(
        log.events().is_empty(),
        "no unit opened: {:?}",
        log.events()
    );
}

// Deleting the commission after a gated declare leaves no Slot node behind.
#[tokio::test]
async fn deleting_the_commission_after_a_gated_declare_leaves_no_slot_node() {
    let world = test_world::world().await;
    let tab = tab_of(&world.ports, untitled()).await;
    declare_by(
        &world.ports,
        &alice(),
        untitled(),
        vec![body(tab, "Gone with it", None)],
    )
    .await
    .expect("the owner declares a Slot");

    let mut unit = world.ports.database.begin().await.expect("begin");
    unit.commissions()
        .delete(&untitled())
        .await
        .expect("the commission is deleted");
    unit.commit().await.expect("commit");

    let slot_nodes = world
        .backend
        .all_nodes()
        .into_iter()
        .filter(|row| row.node_type == NodeType::Slot)
        .count();
    assert_eq!(slot_nodes, 0, "no slot node is left");
    assert!(
        world
            .backend
            .slots_of(untitled())
            .await
            .expect("slots")
            .is_empty()
    );
}

// The use case's Slot and its act print no title and no notes.
#[test]
fn a_declare_s_debug_prints_no_title_or_notes() {
    let tab = TabId::mint();
    let asked = body(tab, CANARY_TITLE, Some(CANARY_NOTES));
    let act = act_of(untitled(), body(tab, CANARY_TITLE, Some(CANARY_NOTES)));

    let printed = [format!("{asked:?}"), format!("{act:?}")];

    for line in printed {
        assert!(!line.contains(CANARY_TITLE), "{line}");
        assert!(!line.contains(CANARY_NOTES), "{line}");
    }
}
