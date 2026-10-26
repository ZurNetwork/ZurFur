//! The commission write port over the in-memory fake: a create makes only the starting
//! state, bound to the gate's node, and refuses an unknown owner or a node that isn't its
//! own; a commit refuses a commission node without its commission; a delete takes the
//! commission's nodes. The fact predicate is reachable only on an open [`UnitOfWork`]'s
//! commissions view and, with no fact-minter wired anywhere, every commission answers
//! `false`. The archive flag rides the same seam: `set_archived` is a transactional write
//! reporting whether the state actually flipped. A commission's name and level are read
//! from its node.

mod common;

use adapter_mem::{CommissionSeed, MemBackend};
use chrono::Utc;
use domain::elements::{
    commission::{
        CommissionBirth, CommissionDeadline, CommissionId, CommissionRole, CommissionStanding,
        CommissionStart, CommissionSummary, CommissionTitle, LockedStanding, NewCommission,
    },
    did::Did,
    level::Level,
    maturity::{Maturity, MaturityRating},
    node::{NodeId, NodeType},
    text::StoredText,
    user::UserId,
};

/// Recognizes `did` as a User on `backend`.
async fn user(backend: &MemBackend, did: &str) -> UserId {
    backend
        .provision(&Did::from(did.to_owned()))
        .await
        .expect("provision the user")
        .id
}

/// Seeds a Private commission titled `title`, owned by `owner`.
async fn seeded(backend: &MemBackend, title: &str, owner: &UserId) -> CommissionSeed {
    let title = title.parse::<CommissionTitle>().expect("a valid title");
    let seed = CommissionSeed::private(title, owner.clone());
    backend
        .seed_bound_commission(&seed)
        .await
        .expect("seed the commission");
    seed
}

/// AC2+AC3 (mem): a freshly created commission holds no facts — asked in the
/// **same unit of work** that created it (the transactional read the delete gate
/// needs), and again from a later unit after the commit.
#[tokio::test]
async fn every_commission_answers_false_with_no_fact_minters_wired() {
    let backend = MemBackend::new();
    let owner = user(&backend, "did:plc:factless-owner").await;
    let db = backend.database();

    // Same-transaction read: create and ask inside one unit of work.
    let mut uow = db.begin().await.expect("begin");
    let (id, _) = common::create_bound(&mut *uow, &owner, Level::Private, "A ref sheet")
        .await
        .expect("create");
    let has_facts = uow
        .commissions()
        .commission_has_facts(&id)
        .await
        .expect("has_facts in the creating unit");
    assert!(
        !has_facts,
        "no fact-minter exists, so no commission can bear facts"
    );
    uow.commit().await.expect("commit");

    // A later unit of work sees the committed commission and the same answer.
    let mut uow = db.begin().await.expect("begin second unit");
    let has_facts = uow
        .commissions()
        .commission_has_facts(&id)
        .await
        .expect("has_facts in a later unit");
    assert!(!has_facts);
    uow.rollback().await.expect("rollback read-only unit");
}

/// A commission id nobody ever created also answers `false`: absence of the
/// commission is absence of facts, not an error — the gate's own existence check
/// is a separate concern.
#[tokio::test]
async fn an_unknown_commission_answers_false() {
    let backend = MemBackend::new();
    let db = backend.database();

    let mut uow = db.begin().await.expect("begin");
    let has_facts = uow
        .commissions()
        .commission_has_facts(&CommissionId::from(uuid::Uuid::now_v7()))
        .await
        .expect("has_facts for an unknown id");
    assert!(!has_facts);
    uow.rollback().await.expect("rollback read-only unit");
}

/// Mem store layer: `set_archived` flips the archive flag through the
/// unit of work and reports **whether the state actually transitioned** — the
/// bool the route keys its changelog append on, so a repeated archive (or
/// un-archive) can never mint a duplicate entry. The first stamp survives a
/// repeat, and both directions round-trip through `find`.
#[tokio::test]
async fn set_archived_round_trips_and_reports_transitions() {
    let backend = MemBackend::new();
    let owner = user(&backend, "did:plc:archiving-owner").await;
    let id = seeded(&backend, "A ref sheet", &owner).await.id;
    assert!(
        backend
            .find_commission(id)
            .await
            .expect("find")
            .expect("present")
            .archived_at
            .is_none(),
        "a fresh commission is active"
    );

    // Archive: a real transition.
    let stamp = Utc::now();
    let mut uow = backend.database().begin().await.expect("begin");
    let changed = uow
        .commissions()
        .set_archived(&id, Some(stamp))
        .await
        .expect("archive");
    assert!(changed, "active -> archived is a transition");
    uow.commit().await.expect("commit");
    let stored = backend
        .find_commission(id)
        .await
        .expect("find")
        .expect("the record survives");
    assert_eq!(stored.archived_at, Some(stamp), "the stamp round-trips");

    // A repeat archive is no transition and keeps the first stamp.
    let mut uow = backend.database().begin().await.expect("begin");
    let changed = uow
        .commissions()
        .set_archived(&id, Some(Utc::now()))
        .await
        .expect("repeat archive");
    assert!(!changed, "archived -> archived is not a transition");
    uow.commit().await.expect("commit");
    assert_eq!(
        backend
            .find_commission(id)
            .await
            .expect("find")
            .expect("present")
            .archived_at,
        Some(stamp),
        "a repeat archive never rewrites the original stamp",
    );

    // Un-archive: back to active.
    let mut uow = backend.database().begin().await.expect("begin");
    let changed = uow
        .commissions()
        .set_archived(&id, None)
        .await
        .expect("unarchive");
    assert!(changed, "archived -> active is a transition");
    uow.commit().await.expect("commit");
    assert!(
        backend
            .find_commission(id)
            .await
            .expect("find")
            .expect("present")
            .archived_at
            .is_none(),
        "un-archiving clears the flag"
    );

    // A repeat un-archive is no transition; an unknown commission is a no-op.
    let mut uow = backend.database().begin().await.expect("begin");
    assert!(
        !uow.commissions()
            .set_archived(&id, None)
            .await
            .expect("repeat unarchive"),
        "active -> active is not a transition",
    );
    assert!(
        !uow.commissions()
            .set_archived(&CommissionId::from(uuid::Uuid::now_v7()), Some(Utc::now()))
            .await
            .expect("archive an unknown id"),
        "an absent commission matches nothing (existence is the caller's check)",
    );
    uow.rollback().await.expect("rollback");
}

/// Mem store layer: an archive staged in a dropped unit of work is
/// discarded — the flag write obeys the same commit-or-discard rule as every
/// other commission write.
#[tokio::test]
async fn a_dropped_unit_of_work_discards_the_archive() {
    let backend = MemBackend::new();
    let owner = user(&backend, "did:plc:rollback-owner").await;
    let id = seeded(&backend, "Kept active", &owner).await.id;

    {
        let mut uow = backend.database().begin().await.expect("begin");
        assert!(
            uow.commissions()
                .set_archived(&id, Some(Utc::now()))
                .await
                .expect("stage archive"),
        );
        // `uow` drops here without `commit` → the staged flag is discarded.
    }

    assert!(
        backend
            .find_commission(id)
            .await
            .expect("find")
            .expect("present")
            .archived_at
            .is_none(),
        "a dropped unit of work archives nothing"
    );
}

/// A create stores the domain's starting state and nothing else. Its title and level are its
/// node's, and only the maturity and deadline are chosen.
#[tokio::test]
async fn a_created_commission_is_stored_in_the_domain_s_starting_state() {
    let backend = MemBackend::new();
    let owner = user(&backend, "did:plc:born-owner").await;
    let id = CommissionId::mint();
    let node = common::commission_node(id, Level::Listed, "A moonlit piece");
    let mut nodes = vec![node.clone()];
    nodes.extend(common::entry_nodes(node.id));
    let created_at = Utc::now();
    let birth = CommissionBirth {
        node: node.id,
        owner_id: owner.clone(),
        created_at,
    };
    let maturity = Maturity {
        rating: MaturityRating::Suggestive,
        graphic: false,
    };
    let deadline = CommissionDeadline::from(created_at + chrono::Duration::days(30));
    let chosen = NewCommission {
        maturity: Some(maturity),
        deadline: Some(deadline),
    };

    let mut unit = backend.database().begin().await.expect("begin");
    common::write_nodes(&mut *unit, &nodes)
        .await
        .expect("write the nodes");
    common::create(&mut *unit, id, &birth, &chosen)
        .await
        .expect("create");
    unit.commit().await.expect("commit");

    let found = backend
        .find_commission(id)
        .await
        .expect("find")
        .expect("the commission is stored");
    let stored_start = CommissionStart {
        lifecycle_step: found.lifecycle_step.clone(),
        dispute: found.dispute.clone(),
        direction_status: found.direction_status,
        delayed_flag: found.delayed_flag,
        linked_channel: found.linked_channel.clone(),
        archived_at: found.archived_at,
    };
    assert_eq!(
        stored_start,
        CommissionStart::default(),
        "the domain's starting state"
    );
    assert_eq!(found.maturity, Some(maturity), "the chosen maturity");
    let chosen_deadline = chrono::DateTime::<Utc>::from(deadline);
    assert_eq!(found.deadline, Some(chosen_deadline), "the chosen deadline");
    assert_eq!(found.owner_id, owner, "the gate's owner");
    assert_eq!(found.created_at, created_at, "the gate's time");
    let summary = backend
        .commission_summary(id)
        .await
        .expect("find")
        .expect("the commission is stored");
    assert_eq!(summary.title.as_str(), "A moonlit piece", "the node's name");
    assert_eq!(summary.visibility, Level::Listed, "the node's level");
}

/// A create at a node the unit can't see errs at the create and spends the unit: a node never
/// written, and one staged only in another unit that hasn't committed.
#[tokio::test]
async fn create_with_an_unseen_node_errs() {
    let backend = MemBackend::new();
    let owner = user(&backend, "did:plc:unseen-owner").await;
    let database = backend.database();

    let id = CommissionId::mint();
    let never_written = CommissionBirth {
        node: NodeId::mint(),
        owner_id: owner.clone(),
        created_at: Utc::now(),
    };
    let mut unit = database.begin().await.expect("begin");
    let created = common::create(&mut *unit, id, &never_written, &common::unrated()).await;
    assert!(created.is_err(), "a node never written is refused");
    let committed = unit.commit().await;
    assert!(committed.is_err(), "the refused create spends the unit");

    let elsewhere = common::commission_node(id, Level::Private, "Staged elsewhere");
    let mut other = database.begin().await.expect("begin another unit");
    common::write_nodes(&mut *other, std::slice::from_ref(&elsewhere))
        .await
        .expect("stage the node in the other unit");
    let staged_elsewhere = CommissionBirth {
        node: elsewhere.id,
        ..never_written
    };
    let mut unit = database.begin().await.expect("begin");
    let created = common::create(&mut *unit, id, &staged_elsewhere, &common::unrated()).await;
    assert!(
        created.is_err(),
        "a node another open unit staged is refused"
    );
    drop(other);
    assert!(
        backend.find_commission(id).await.expect("find").is_none(),
        "no commission is stored"
    );
}

/// A create at a node that isn't its own errs at the create and spends the unit: another
/// commission's node, an entry node, and a top-level commission node whose segment isn't the id.
#[tokio::test]
async fn create_with_a_node_that_is_not_its_own_errs() {
    let backend = MemBackend::new();
    let owner = user(&backend, "did:plc:misbound-owner").await;
    let database = backend.database();
    let mut unit = database.begin().await.expect("begin");
    let (_, other_node) = common::create_bound(&mut *unit, &owner, Level::Private, "Another")
        .await
        .expect("another commission");
    unit.commit().await.expect("commit the other commission");
    let id = CommissionId::mint();
    let own = common::commission_node(id, Level::Private, "Its own");
    let entries = common::entry_nodes(own.id);
    let stray = common::commission_node(CommissionId::mint(), Level::Private, "A stray");
    let mut nodes = vec![own, stray.clone()];
    nodes.extend(entries.clone());

    let brief = entries
        .iter()
        .find(|entry| entry.node_type == NodeType::CommissionBrief)
        .expect("the brief entry")
        .id;
    let wrong_nodes = [
        ("another commission's node", other_node),
        ("an entry node", brief),
        ("a node whose segment isn't the id", stray.id),
    ];
    for (what, wrong) in wrong_nodes {
        let mut unit = database.begin().await.expect("begin");
        common::write_nodes(&mut *unit, &nodes)
            .await
            .expect("write the nodes");
        let birth = CommissionBirth {
            node: wrong,
            owner_id: owner.clone(),
            created_at: Utc::now(),
        };
        let created = common::create(&mut *unit, id, &birth, &common::unrated()).await;
        assert!(created.is_err(), "{what} is refused at the create");
        let committed = unit.commit().await;
        assert!(
            committed.is_err(),
            "{what}: the refused create spends the unit"
        );
    }
    assert_eq!(
        backend.all_nodes().len(),
        7,
        "only the other commission's nodes"
    );
}

/// A create owned by an id no User holds errs at the create and spends the unit.
#[tokio::test]
async fn create_refuses_an_unknown_owner() {
    let backend = MemBackend::new();
    let stranger = UserId::from(Did::from("did:plc:nobody-here".to_owned()));
    let id = CommissionId::mint();
    let node = common::commission_node(id, Level::Private, "Ownerless");

    let mut unit = backend.database().begin().await.expect("begin");
    common::write_nodes(&mut *unit, std::slice::from_ref(&node))
        .await
        .expect("write the node");
    let birth = CommissionBirth {
        node: node.id,
        owner_id: stranger,
        created_at: Utc::now(),
    };
    let created = common::create(&mut *unit, id, &birth, &common::unrated()).await;

    assert!(created.is_err(), "an unknown owner is refused");
    let committed = unit.commit().await;
    assert!(committed.is_err(), "the refused create spends the unit");
    assert!(backend.all_nodes().is_empty(), "no node is kept");
}

/// A unit that commits a commission node with no commission bound to it is refused, and
/// keeps nothing.
#[tokio::test]
async fn a_commission_node_without_its_commission_is_refused_at_commit() {
    let backend = MemBackend::new();
    let node = common::commission_node(CommissionId::mint(), Level::Private, "A phantom");
    let mut nodes = vec![node.clone()];
    nodes.extend(common::entry_nodes(node.id));

    let mut unit = backend.database().begin().await.expect("begin");
    common::write_nodes(&mut *unit, &nodes)
        .await
        .expect("write the nodes");
    let committed = unit.commit().await;

    assert!(committed.is_err(), "the phantom node is refused");
    assert!(backend.all_nodes().is_empty(), "no node is kept");
}

/// A unit that deletes a commission's node but keeps the commission is refused at commit.
#[tokio::test]
async fn a_commission_whose_node_is_gone_is_refused_at_commit() {
    let backend = MemBackend::new();
    let owner = user(&backend, "did:plc:nodeless-owner").await;
    let seed = seeded(&backend, "Unmoored", &owner).await;
    let node = backend
        .node_store()
        .at_top(
            &NodeType::Commission,
            &seed.id.to_string().parse().expect("a segment"),
        )
        .await
        .expect("a read")
        .expect("the seeded node");

    let mut unit = backend.database().begin().await.expect("begin");
    let deleted = unit.nodes().delete(&node.id).await.expect("delete");
    assert!(deleted, "the node was there");
    let committed = unit.commit().await;

    assert!(
        committed.is_err(),
        "the commission without its node is refused"
    );
    assert_eq!(backend.all_nodes().len(), 7, "every node stays");
}

/// Deleting a commission removes its node and every node under it; no node keeps its
/// title, and another commission's nodes stay.
#[tokio::test]
async fn deleting_a_commission_removes_its_nodes() {
    let backend = MemBackend::new();
    let owner = user(&backend, "did:plc:deleting-owner").await;
    let doomed = seeded(&backend, "Doomed piece", &owner).await;
    let survivor = seeded(&backend, "Surviving piece", &owner).await;
    assert_eq!(backend.all_nodes().len(), 14, "two trees of seven nodes");

    let mut unit = backend.database().begin().await.expect("begin");
    unit.commissions().delete(&doomed.id).await.expect("delete");
    unit.commit().await.expect("commit");

    let nodes = backend.all_nodes();
    assert_eq!(nodes.len(), 7, "only the survivor's tree stays");
    assert!(
        nodes
            .iter()
            .all(|node| node.name.as_str() != "Doomed piece"),
        "no node carries the deleted title"
    );
    let doomed_segment = doomed.id.to_string();
    assert!(
        nodes
            .iter()
            .all(|node| node.segment.as_ref() != doomed_segment),
        "the deleted commission's node is gone"
    );
    assert!(
        nodes
            .iter()
            .any(|node| node.segment.as_ref() == survivor.id.to_string()),
        "the survivor's node stays"
    );
}

// The summary, the participant listing and both standing reads take the commission's name and
// level from its node: a rename of the node alone is what they read.
#[tokio::test]
async fn the_summary_and_standing_read_the_node_s_name_and_level() {
    let backend = MemBackend::new();
    let owner = user(&backend, "did:plc:node-reads-owner").await;
    let seed = CommissionSeed {
        level: Level::Listed,
        ..CommissionSeed::private("Untitled".parse().expect("a valid title"), owner.clone())
    };
    let node = backend
        .seed_bound_commission(&seed)
        .await
        .expect("seed the commission");
    let mut unit = backend.database().begin().await.expect("begin");
    let renamed = unit
        .nodes()
        .set_name(&node, &StoredText::from("The node's name".to_owned()))
        .await
        .expect("rename the node");
    unit.commit().await.expect("commit the rename");
    assert!(renamed, "the node is renamed");
    let store = backend.commission_store();

    let summary = store.find_summary(&seed.id).await.expect("find_summary");
    let listing = store
        .list_participating(&owner)
        .await
        .expect("list_participating");
    let standing = store.standing(&seed.id, &owner).await.expect("standing");
    let mut unit = backend.database().begin().await.expect("begin");
    let locked = unit
        .commissions()
        .standing_for_update(&seed.id, &owner)
        .await
        .expect("standing_for_update");

    let expected_summary = CommissionSummary {
        id: seed.id,
        title: StoredText::from("The node's name".to_owned()),
        visibility: Level::Listed,
        archived: false,
    };
    let expected_standing = CommissionStanding {
        role: Some(CommissionRole::Owner),
        level: Level::Listed,
        archived: false,
    };
    assert_eq!(summary, Some(expected_summary.clone()));
    assert_eq!(listing, [expected_summary]);
    let expected_locked = LockedStanding {
        standing: expected_standing,
        node,
    };
    assert_eq!(standing, Some(expected_standing));
    assert_eq!(locked, Some(expected_locked));
}
