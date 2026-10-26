//! The shared node conformance suite, run by both adapters: `adapter-mem`'s
//! `tests/node.rs` and `adapter-pg`'s `tests/it/node.rs`.
//!
//! Each case takes a fresh [`Database`], the [`NodeStore`] over the same store, the
//! adapter's [`CommissionSeeder`] and its [`SlotSeeder`], and builds 0.2's tree: a commission
//! node, its six entries, Slots under `slots`. Every commission node a case commits is bound
//! to its commission, so its segment is a commission id; every slot node a case commits is
//! bound to its Slot, so its segment is an element id. A slot node no Slot points at lives
//! only inside an uncommitted unit.
//! [`node_contract_tests!`](crate::node_contract_tests) writes one test per case.
//! The suite holds the port's promise that a refusal ends the unit: every later
//! call on it fails, and its commit returns an error and keeps nothing.

use chrono::Utc;
use domain::elements::commission::{CommissionId, NewSlot, SlotBirth, SlotDeclaration};
use domain::elements::level::Level;
use domain::elements::node::{NewNode, NodeId, NodeRow, NodeSegment, NodeType};
use domain::elements::text::StoredText;
use domain::ports::{Database, NodeStore, SegmentTaken, UnitOfWork};
use strum::VariantArray;

use crate::seeder::{CommissionSeeder, SlotSeeder, bind, made_up_owner};

/// The six fixed entries, in the order a commission's are written.
pub const ENTRY_TYPES: [NodeType; 6] = [
    NodeType::CommissionAttachments,
    NodeType::CommissionProducts,
    NodeType::CommissionSlots,
    NodeType::CommissionChangelog,
    NodeType::CommissionParticipants,
    NodeType::CommissionBrief,
];

/// Writes one `#[tokio::test]` per suite case. `$setup` is an async fn returning
/// `(Arc<dyn Database>, Arc<dyn NodeStore>, Arc<dyn CommissionSeeder>, Arc<dyn SlotSeeder>,
/// impl Sized)` over one fresh store; the fifth item is kept alive for the test.
#[macro_export]
macro_rules! node_contract_tests {
    ($setup:path) => {
        $crate::node_contract_tests!(@cases $setup;
            create_then_child_reads_back,
            at_top_filters_by_type,
            at_top_reads_back_the_stored_name_and_level,
            every_node_type_is_storable,
            a_second_top_level_row_with_the_same_segment_is_refused,
            a_primary_key_collision_is_not_segment_taken,
            a_write_after_segment_taken_fails_on_the_same_unit,
            a_commit_after_segment_taken_fails_and_keeps_nothing,
            the_same_segment_under_two_parents_is_allowed,
            a_child_of_a_missing_parent_is_refused,
            children_come_in_segment_order,
            delete_takes_the_subtree,
            deleting_a_commission_node_takes_its_entries,
            a_node_that_does_not_fit_its_parent_is_refused,
            deleting_a_fixed_entry_is_refused_while_its_commission_lives,
            set_name_and_delete_of_a_missing_id_answer_false,
            set_name_changes_only_the_name,
            an_empty_name_is_refused,
            a_rolled_back_unit_keeps_no_node,
            a_slot_node_without_its_slot_is_refused_at_commit,
        );
    };
    (@cases $setup:path; $($case:ident),* $(,)?) => {
        $(
            #[tokio::test]
            async fn $case() {
                let (database, store, seeder, slots, _kept) = $setup().await;
                let seeders = $crate::node_contract::Seeders {
                    commissions: &*seeder,
                    slots: &*slots,
                };
                $crate::node_contract::$case(&*database, &*store, seeders).await;
            }
        )*
    };
}

/// A segment from made-up text.
fn segment(text: &str) -> NodeSegment {
    NodeSegment::try_from(text).expect("a made-up segment follows the rule")
}

/// A stored name from made-up text.
fn name(text: &str) -> StoredText {
    StoredText::from(text.to_owned())
}

/// A fresh commission segment: a new commission id's text, as a commission's own segment is.
fn fresh_segment() -> NodeSegment {
    segment(&CommissionId::mint().to_string())
}

/// The adapter's two seeders: of commissions, and of Slots on committed commissions.
#[derive(Clone, Copy)]
pub struct Seeders<'a> {
    /// Creates a commission through the port.
    pub commissions: &'a dyn CommissionSeeder,
    /// Declares a Slot through the port.
    pub slots: &'a dyn SlotSeeder,
}

/// A top-level commission node holding `top`.
fn commission_node(top: &NodeSegment) -> NewNode {
    NewNode {
        id: NodeId::mint(),
        parent: None,
        segment: top.clone(),
        node_type: NodeType::Commission,
        level: Level::Private,
        name: name("A made-up commission"),
    }
}

/// The fixed entry `entry_type` under `parent`, its word as its segment and name.
pub(crate) fn entry_node(parent: NodeId, entry_type: NodeType) -> NewNode {
    let word: &'static str = entry_type.into();
    let word = word
        .strip_prefix("commission.")
        .expect("an entry's type is commission.<word>");
    NewNode {
        id: NodeId::mint(),
        parent: Some(parent),
        segment: segment(word),
        node_type: entry_type,
        level: Level::Private,
        name: name(word),
    }
}

/// A Slot node holding `slot_segment` under the `slots` entry `parent`.
fn slot_node(parent: NodeId, slot_segment: &str) -> NewNode {
    NewNode {
        id: NodeId::mint(),
        parent: Some(parent),
        segment: segment(slot_segment),
        node_type: NodeType::Slot,
        level: Level::Private,
        name: name("A made-up slot"),
    }
}

/// One commission's nodes as written, and the commission bound to them.
pub struct Tree {
    /// The commission bound to the commission node.
    pub commission_id: CommissionId,
    /// The commission node.
    pub commission: NewNode,
    /// Its six entries, in [`ENTRY_TYPES`] order.
    pub entries: Vec<NewNode>,
    /// Its Slots' nodes, under its `slots` entry.
    pub slots: Vec<NewNode>,
}

impl Tree {
    /// The commission's `slots` entry.
    pub fn slots_entry(&self) -> &NewNode {
        self.entry(NodeType::CommissionSlots)
    }

    /// The commission's entry of `entry_type`.
    pub fn entry(&self, entry_type: NodeType) -> &NewNode {
        self.entries
            .iter()
            .find(|entry| entry.node_type == entry_type)
            .expect("one of the six entries")
    }

    /// Every node of the tree.
    pub fn every_node(&self) -> impl Iterator<Item = &NewNode> {
        std::iter::once(&self.commission)
            .chain(&self.entries)
            .chain(&self.slots)
    }
}

/// Writes 0.2's tree on `unit`: a commission node holding `top`, a commission id's text,
/// its six entries, one Slot node per `slot_segments` under its `slots` entry, and the
/// commission bound to it.
pub async fn write_tree(
    unit: &mut dyn UnitOfWork,
    seeder: &dyn CommissionSeeder,
    top: &NodeSegment,
    slot_segments: &[&str],
) -> Tree {
    let commission = commission_node(top);
    let entries: Vec<NewNode> = ENTRY_TYPES
        .iter()
        .map(|entry_type| entry_node(commission.id, *entry_type))
        .collect();
    let slots_entry = entries
        .iter()
        .find(|entry| entry.node_type == NodeType::CommissionSlots)
        .expect("the slots entry")
        .id;
    let slots: Vec<NewNode> = slot_segments
        .iter()
        .map(|slot_segment| slot_node(slots_entry, slot_segment))
        .collect();
    {
        let mut nodes = unit.nodes();
        for node in std::iter::once(&commission).chain(&entries).chain(&slots) {
            nodes
                .create(node)
                .await
                .expect("a node of the tree is stored");
        }
    }
    let owner = made_up_owner(unit).await;
    let commission_id = bind(unit, seeder, &commission, &owner).await;
    Tree {
        commission_id,
        commission,
        entries,
        slots,
    }
}

/// Writes 0.2's tree in a unit of its own and commits it, then `slot_count` Slots under its
/// `slots` entry in a second unit, each node bound to its Slot, keyed by its element id.
async fn committed_tree(database: &dyn Database, seeders: Seeders<'_>, slot_count: usize) -> Tree {
    let mut unit = database.begin().await.expect("begin");
    let mut tree = write_tree(&mut *unit, seeders.commissions, &fresh_segment(), &[]).await;
    unit.commit().await.expect("the tree commits");
    if slot_count > 0 {
        let mut unit = database.begin().await.expect("begin");
        let slots = write_slots(&mut *unit, seeders.slots, &tree, slot_count).await;
        unit.commit().await.expect("the Slots commit");
        tree.slots = slots;
    }
    tree
}

/// A fresh Slot segment: a new element id's text, as a Slot's own segment is.
fn fresh_slot_segment() -> NodeSegment {
    let declaration = SlotDeclaration::minted(NewSlot {
        address: domain::elements::commission::SurfaceAddress::new(
            domain::elements::commission::TabId::mint(),
            "content".parse().expect("a surface name"),
        ),
        notes: None,
    });
    segment(&declaration.element.to_string())
}

/// Writes on `unit` `count` Slot nodes under the committed `tree`'s `slots` entry, each keyed
/// by a fresh element id and bound to its Slot through `slots`. Answers the nodes.
async fn write_slots(
    unit: &mut dyn UnitOfWork,
    slots: &dyn SlotSeeder,
    tree: &Tree,
    count: usize,
) -> Vec<NewNode> {
    let owner = made_up_owner(unit).await;
    let mut written = Vec::new();
    for _ in 0..count {
        let node = slot_node(tree.slots_entry().id, fresh_slot_segment().as_ref());
        unit.nodes()
            .create(&node)
            .await
            .expect("a slot node is stored");
        let element = node
            .segment
            .as_ref()
            .parse()
            .expect("a slot node's segment is an element id");
        let birth = SlotBirth {
            element,
            node: node.id,
            created_by: owner.clone(),
            created_at: Utc::now(),
        };
        slots
            .declare_slot(unit, tree.commission_id, &birth)
            .await
            .expect("the Slot is declared at its node");
        written.push(node);
    }
    written
}

/// Asserts `read` is `node` as written, field by field.
fn assert_reads_back(read: Option<NodeRow>, node: &NewNode) {
    let read = read.expect("the node reads back");
    assert_eq!(read.id, node.id, "id");
    assert_eq!(read.parent, node.parent, "parent");
    assert_eq!(read.segment, node.segment, "segment");
    assert_eq!(read.node_type, node.node_type, "type");
    assert_eq!(read.level, node.level, "level");
    assert_eq!(read.name, node.name, "name");
}

/// Asserts the create of `node` in a fresh unit is refused, and that the refusal is no
/// [`SegmentTaken`].
async fn assert_refused_not_taken(database: &dyn Database, node: &NewNode, what: &str) {
    let mut unit = database.begin().await.expect("begin");
    let created = unit.nodes().create(node).await;
    let error = created.expect_err(what);
    assert!(
        error.downcast_ref::<SegmentTaken>().is_none(),
        "{what}: refused, but not as a taken segment"
    );
}

/// A created node reads back by its parent and segment, on the unit and after commit; a Slot's
/// node too, committed with its Slot.
pub async fn create_then_child_reads_back(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let mut unit = database.begin().await.expect("begin");
    let mut tree = write_tree(&mut *unit, seeders.commissions, &fresh_segment(), &[]).await;
    let brief = tree.entry(NodeType::CommissionBrief).clone();

    let on_the_unit = unit
        .nodes()
        .child(&tree.commission.id, &brief.segment)
        .await
        .expect("a read on the unit");
    assert_reads_back(on_the_unit, &brief);
    unit.commit().await.expect("commit");
    let mut unit = database.begin().await.expect("begin");
    tree.slots = write_slots(&mut *unit, seeders.slots, &tree, 1).await;
    let slot = &tree.slots[0];
    let slot_on_the_unit = unit
        .nodes()
        .child(&tree.slots_entry().id, &slot.segment)
        .await
        .expect("a read on the unit");
    assert_reads_back(slot_on_the_unit, slot);
    unit.commit().await.expect("commit");

    let brief_read = store
        .child(&tree.commission.id, &brief.segment)
        .await
        .expect("a read");
    assert_reads_back(brief_read, &brief);
    let slot_read = store
        .child(&tree.slots_entry().id, &slot.segment)
        .await
        .expect("a read");
    assert_reads_back(slot_read, slot);
    let commission_read = store
        .at_top(&NodeType::Commission, &tree.commission.segment)
        .await
        .expect("a read");
    assert_reads_back(commission_read, &tree.commission);
}

/// `at_top` finds a top-level node only under the type asked for.
pub async fn at_top_filters_by_type(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 0).await;
    let top = &tree.commission.segment;

    for other in NodeType::VARIANTS
        .iter()
        .filter(|node_type| **node_type != NodeType::Commission)
    {
        let read = store.at_top(other, top).await.expect("a read");
        assert!(
            read.is_none(),
            "no top-level {other} node holds the segment"
        );
    }
    let mut unit = database.begin().await.expect("begin");
    let on_the_unit = unit
        .nodes()
        .at_top(&NodeType::CommissionSlots, top)
        .await
        .expect("a read on the unit");
    assert!(on_the_unit.is_none(), "the unit's read filters by type too");
    let found = store
        .at_top(&NodeType::Commission, top)
        .await
        .expect("a read");
    assert_reads_back(found, &tree.commission);
}

/// A top-level node's stored name and level read back as written, not as defaults.
pub async fn at_top_reads_back_the_stored_name_and_level(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let top = fresh_segment();
    let listed = NewNode {
        level: Level::Listed,
        name: name("Moonlit made-up piece"),
        ..commission_node(&top)
    };
    let public_top = fresh_segment();
    let public = NewNode {
        level: Level::Public,
        name: name("Another made-up piece"),
        ..commission_node(&public_top)
    };
    let mut unit = database.begin().await.expect("begin");
    {
        let mut nodes = unit.nodes();
        nodes.create(&listed).await.expect("create");
        nodes.create(&public).await.expect("create");
    }
    let owner = made_up_owner(&mut *unit).await;
    bind(&mut *unit, seeders.commissions, &listed, &owner).await;
    bind(&mut *unit, seeders.commissions, &public, &owner).await;
    unit.commit().await.expect("commit");

    let listed_read = store
        .at_top(&NodeType::Commission, &top)
        .await
        .expect("a read");
    assert_reads_back(listed_read, &listed);
    let public_read = store
        .at_top(&NodeType::Commission, &public_top)
        .await
        .expect("a read");
    assert_reads_back(public_read, &public);
}

/// A node of every type the vocabulary holds is stored and reads back with its type.
pub async fn every_node_type_is_storable(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 1).await;

    for node_type in NodeType::VARIANTS {
        let written = tree
            .every_node()
            .find(|node| node.node_type == *node_type)
            .unwrap_or_else(|| panic!("the tree holds a {node_type} node"));
        let read = match written.parent {
            None => store.at_top(node_type, &written.segment).await,
            Some(parent) => store.child(&parent, &written.segment).await,
        };
        let read = read.expect("a read");
        assert_reads_back(read, written);
    }
}

/// A second top-level node with a taken segment is refused as [`SegmentTaken`].
pub async fn a_second_top_level_row_with_the_same_segment_is_refused(
    database: &dyn Database,
    _store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 0).await;
    let second = commission_node(&tree.commission.segment);

    let mut unit = database.begin().await.expect("begin");
    let created = unit.nodes().create(&second).await;

    let error = created.expect_err("a taken top-level segment is refused");
    assert!(
        error.downcast_ref::<SegmentTaken>().is_some(),
        "the refusal is SegmentTaken"
    );
}

/// A node whose id is taken is refused, and that refusal is no [`SegmentTaken`].
pub async fn a_primary_key_collision_is_not_segment_taken(
    database: &dyn Database,
    _store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 0).await;
    let same_id = NewNode {
        id: tree.commission.id,
        ..commission_node(&fresh_segment())
    };

    assert_refused_not_taken(database, &same_id, "a taken id is refused").await;
}

/// After a refusal, every later call on the same unit fails, a valid one included.
pub async fn a_write_after_segment_taken_fails_on_the_same_unit(
    database: &dyn Database,
    _store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 0).await;
    let mut unit = database.begin().await.expect("begin");
    let refused = unit
        .nodes()
        .create(&commission_node(&tree.commission.segment))
        .await;
    assert!(refused.is_err(), "the taken segment is refused");

    let valid = unit
        .nodes()
        .create(&commission_node(&fresh_segment()))
        .await;
    assert!(valid.is_err(), "a valid create after the refusal fails");
    let read = unit
        .nodes()
        .at_top(&NodeType::Commission, &tree.commission.segment)
        .await;
    assert!(read.is_err(), "a read after the refusal fails");
    let renamed = unit
        .nodes()
        .set_name(&tree.commission.id, &name("Renamed"))
        .await;
    assert!(renamed.is_err(), "a rename after the refusal fails");
}

/// After a refusal, commit fails and nothing the unit wrote is kept.
pub async fn a_commit_after_segment_taken_fails_and_keeps_nothing(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 0).await;
    let kept_before = commission_node(&fresh_segment());
    let mut unit = database.begin().await.expect("begin");
    unit.nodes()
        .create(&kept_before)
        .await
        .expect("a valid create before the refusal");
    let refused = unit
        .nodes()
        .create(&commission_node(&tree.commission.segment))
        .await;
    assert!(refused.is_err(), "the taken segment is refused");

    let committed = unit.commit().await;

    assert!(committed.is_err(), "the commit fails");
    let read = store
        .at_top(&NodeType::Commission, &kept_before.segment)
        .await
        .expect("a read");
    assert!(
        read.is_none(),
        "the node written before the refusal is not kept"
    );
}

/// One segment is held under two parents: an entry word, committed, and a Slot's segment, on
/// one unit (two Slots never share an element id, so never a committed segment).
pub async fn the_same_segment_under_two_parents_is_allowed(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let first = committed_tree(database, seeders, 0).await;
    let second = committed_tree(database, seeders, 0).await;

    for tree in [&first, &second] {
        let brief = tree.entry(NodeType::CommissionBrief);
        let brief_read = store
            .child(&tree.commission.id, &brief.segment)
            .await
            .expect("a read");
        assert_reads_back(brief_read, brief);
    }
    let mut unit = database.begin().await.expect("begin");
    let one = write_tree(
        &mut *unit,
        seeders.commissions,
        &fresh_segment(),
        &["shared-slot"],
    )
    .await;
    let other = write_tree(
        &mut *unit,
        seeders.commissions,
        &fresh_segment(),
        &["shared-slot"],
    )
    .await;
    for tree in [&one, &other] {
        let slot_read = unit
            .nodes()
            .child(&tree.slots_entry().id, &segment("shared-slot"))
            .await
            .expect("a read on the unit");
        assert_reads_back(slot_read, &tree.slots[0]);
    }
}

/// A node under a parent that doesn't exist is refused, and nothing is kept.
pub async fn a_child_of_a_missing_parent_is_refused(
    database: &dyn Database,
    store: &dyn NodeStore,
    _seeders: Seeders<'_>,
) {
    let missing = NodeId::mint();
    let orphans = [
        entry_node(missing, NodeType::CommissionBrief),
        slot_node(missing, "orphan-slot"),
    ];

    for orphan in &orphans {
        assert_refused_not_taken(database, orphan, "a child of a missing parent is refused").await;
    }
    let read = store.children(&missing).await.expect("a read");
    assert!(read.is_empty(), "no orphan is kept");
}

/// Children come bytewise by segment, so `B` sorts before `a`: a Slot's on the unit, since a
/// slot node commits only with its Slot, and the entries after commit.
pub async fn children_come_in_segment_order(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 0).await;

    let entries = store.children(&tree.commission.id).await.expect("a read");

    let entry_segments: Vec<&str> = entries.iter().map(|node| node.segment.as_ref()).collect();
    assert_eq!(
        entry_segments,
        [
            "attachments",
            "brief",
            "changelog",
            "participants",
            "products",
            "slots"
        ]
    );
    let mut unit = database.begin().await.expect("begin");
    let unit_tree = write_tree(
        &mut *unit,
        seeders.commissions,
        &fresh_segment(),
        &["b", "a", "B", "_", "0", "~", "A"],
    )
    .await;
    let on_the_unit = unit
        .nodes()
        .children(&unit_tree.slots_entry().id)
        .await
        .expect("a read on the unit");
    let unit_segments: Vec<&str> = on_the_unit
        .iter()
        .map(|node| node.segment.as_ref())
        .collect();
    assert_eq!(unit_segments, ["0", "A", "B", "_", "a", "b", "~"]);
}

/// Deleting a commission node takes every node under it, the Slots under `slots` included;
/// its commission goes in the same unit, since a commission never outlives its node.
pub async fn delete_takes_the_subtree(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 2).await;
    let other = committed_tree(database, seeders, 1).await;

    let mut unit = database.begin().await.expect("begin");
    let deleted = unit
        .nodes()
        .delete(&tree.commission.id)
        .await
        .expect("delete");
    unit.commissions()
        .delete(&tree.commission_id)
        .await
        .expect("the commission goes with its node");
    unit.commit().await.expect("commit");

    assert!(deleted, "the commission node was there");
    let slots = store
        .children(&tree.slots_entry().id)
        .await
        .expect("a read");
    assert!(slots.is_empty(), "the Slot nodes went with it");
    let commission = store
        .at_top(&NodeType::Commission, &tree.commission.segment)
        .await
        .expect("a read");
    assert!(commission.is_none(), "the commission node is gone");
    let untouched = store
        .children(&other.slots_entry().id)
        .await
        .expect("a read");
    assert_eq!(untouched.len(), 1, "another commission's tree stays");
}

/// Deleting a commission node, its commission with it, takes its six entries.
pub async fn deleting_a_commission_node_takes_its_entries(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 0).await;

    let mut unit = database.begin().await.expect("begin");
    unit.nodes()
        .delete(&tree.commission.id)
        .await
        .expect("delete");
    unit.commissions()
        .delete(&tree.commission_id)
        .await
        .expect("the commission goes with its node");
    unit.commit().await.expect("commit");

    let entries = store.children(&tree.commission.id).await.expect("a read");
    assert!(entries.is_empty(), "the entries went with it");
    for entry in &tree.entries {
        let read = store
            .child(&tree.commission.id, &entry.segment)
            .await
            .expect("a read");
        assert!(read.is_none(), "the {} entry is gone", entry.node_type);
    }
}

/// A node that doesn't fit where it is put is refused, each refusal no [`SegmentTaken`].
pub async fn a_node_that_does_not_fit_its_parent_is_refused(
    database: &dyn Database,
    _store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 1).await;
    let brief = tree.entry(NodeType::CommissionBrief).id;
    let slots_entry = tree.slots_entry().id;
    let slot = tree.slots[0].id;
    let misfits = [
        (
            "a slot directly under a commission",
            slot_node(tree.commission.id, "loose-slot"),
        ),
        (
            "a slot under the brief entry",
            slot_node(brief, "loose-slot"),
        ),
        ("a slot under a slot", slot_node(slot, "loose-slot")),
        (
            "an entry under the slots entry",
            entry_node(slots_entry, NodeType::CommissionBrief),
        ),
        (
            "an entry under a slot",
            entry_node(slot, NodeType::CommissionBrief),
        ),
        (
            "an entry at the top",
            NewNode {
                parent: None,
                ..entry_node(tree.commission.id, NodeType::CommissionBrief)
            },
        ),
        (
            "a slot at the top",
            NewNode {
                parent: None,
                ..slot_node(slots_entry, "loose-slot")
            },
        ),
        (
            "a commission under a commission",
            NewNode {
                parent: Some(tree.commission.id),
                ..commission_node(&fresh_segment())
            },
        ),
        (
            "an entry whose segment is not its word",
            NewNode {
                segment: segment("notes"),
                ..entry_node(tree.commission.id, NodeType::CommissionBrief)
            },
        ),
    ];

    for (what, misfit) in &misfits {
        assert_refused_not_taken(database, misfit, what).await;
    }
}

/// A fixed entry can't be deleted while its commission node lives; it stays.
pub async fn deleting_a_fixed_entry_is_refused_while_its_commission_lives(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 0).await;

    for entry in &tree.entries {
        let mut unit = database.begin().await.expect("begin");
        let deleted = unit.nodes().delete(&entry.id).await;
        assert!(
            deleted.is_err(),
            "deleting the {} entry is refused",
            entry.node_type
        );
        let committed = unit.commit().await;
        assert!(committed.is_err(), "the refused unit doesn't commit");
    }
    let entries = store.children(&tree.commission.id).await.expect("a read");
    assert_eq!(entries.len(), 6, "every entry stays");
}

/// Renaming or deleting an id no node holds answers `false` and ends nothing.
pub async fn set_name_and_delete_of_a_missing_id_answer_false(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let missing = NodeId::mint();
    let tree = committed_tree(database, seeders, 0).await;

    let mut unit = database.begin().await.expect("begin");
    let renamed = unit
        .nodes()
        .set_name(&missing, &name("Renamed"))
        .await
        .expect("a rename of a missing id is no error");
    let deleted = unit
        .nodes()
        .delete(&missing)
        .await
        .expect("a delete of a missing id is no error");
    let renamed_real = unit
        .nodes()
        .set_name(&tree.commission.id, &name("Renamed for real"))
        .await
        .expect("the unit is still open");
    unit.commit().await.expect("the unit still commits");

    assert!(!renamed, "a rename of a missing id answers false");
    assert!(!deleted, "a delete of a missing id answers false");
    assert!(renamed_real, "a rename of a stored id answers true");
    let read = store
        .at_top(&NodeType::Commission, &tree.commission.segment)
        .await
        .expect("a read")
        .expect("the commission node");
    assert_eq!(read.name, name("Renamed for real"));
}

/// A rename changes the name and nothing else.
pub async fn set_name_changes_only_the_name(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 1).await;
    let slot = &tree.slots[0];
    let new_name = name("A renamed made-up slot");

    let mut unit = database.begin().await.expect("begin");
    let renamed = unit
        .nodes()
        .set_name(&slot.id, &new_name)
        .await
        .expect("rename");
    unit.commit().await.expect("commit");

    assert!(renamed, "the Slot node was there");
    let expected = NewNode {
        name: new_name,
        ..slot.clone()
    };
    let read = store
        .child(&tree.slots_entry().id, &slot.segment)
        .await
        .expect("a read");
    assert_reads_back(read, &expected);
    let commission = store
        .at_top(&NodeType::Commission, &tree.commission.segment)
        .await
        .expect("a read");
    assert_reads_back(commission, &tree.commission);
}

/// An empty name is refused on create and on rename.
pub async fn an_empty_name_is_refused(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 0).await;
    let nameless = NewNode {
        name: name(""),
        ..commission_node(&fresh_segment())
    };
    assert_refused_not_taken(database, &nameless, "an empty name is refused").await;

    let mut unit = database.begin().await.expect("begin");
    let renamed = unit.nodes().set_name(&tree.commission.id, &name("")).await;
    assert!(renamed.is_err(), "a rename to an empty name is refused");
    let committed = unit.commit().await;
    assert!(committed.is_err(), "the refused unit doesn't commit");
    let read = store
        .at_top(&NodeType::Commission, &tree.commission.segment)
        .await
        .expect("a read");
    assert_reads_back(read, &tree.commission);
}

/// A unit rolled back or dropped keeps none of its nodes.
pub async fn a_rolled_back_unit_keeps_no_node(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let rolled_back_top = fresh_segment();
    let mut unit = database.begin().await.expect("begin");
    write_tree(
        &mut *unit,
        seeders.commissions,
        &rolled_back_top,
        &["slot-one"],
    )
    .await;
    unit.rollback().await.expect("rollback");

    let dropped_top = fresh_segment();
    let mut unit = database.begin().await.expect("begin");
    write_tree(&mut *unit, seeders.commissions, &dropped_top, &["slot-one"]).await;
    drop(unit);

    for top in [&rolled_back_top, &dropped_top] {
        let read = store
            .at_top(&NodeType::Commission, top)
            .await
            .expect("a read");
        assert!(read.is_none(), "no node of the unit is kept");
    }
}

/// A slot node no Slot points at is refused at commit, and nothing of its unit is kept.
pub async fn a_slot_node_without_its_slot_is_refused_at_commit(
    database: &dyn Database,
    store: &dyn NodeStore,
    seeders: Seeders<'_>,
) {
    let tree = committed_tree(database, seeders, 0).await;
    let phantom = slot_node(tree.slots_entry().id, fresh_slot_segment().as_ref());
    let mut unit = database.begin().await.expect("begin");
    unit.nodes()
        .create(&phantom)
        .await
        .expect("a slot node is written on the unit");

    let committed = unit.commit().await;

    assert!(
        committed.is_err(),
        "a slot node without its slot is refused"
    );
    let read = store
        .child(&tree.slots_entry().id, &phantom.segment)
        .await
        .expect("a read");
    assert!(read.is_none(), "the slot node is not kept");
}
