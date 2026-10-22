use std::sync::Arc;

use adapter_mem::{
    CommissionSeed,
    fixture::{alice, bob, old_sketch, untitled, untitled_files, untitled_slots},
};
use domain::elements::commission::{CommissionTitle, FileKey, FileName};
use test_support::recording::{CallLog, RecordingCommissionStore, RecordingFileStore};
use uuid::Uuid;

use super::*;
use crate::{
    den::{DenAccess, DenContent, DenError, DenNode, NodeKind, SoftDelete},
    test_world::{self, World, path, segment},
};

/// The real path of a commission.
fn commission_path(commission: CommissionId) -> atta::RealPath {
    path(&format!("/commission/{commission}"))
}

/// Each entry's name, in order.
fn names(content: &Option<DenContent>) -> Vec<String> {
    let Some(DenContent::Listing(listing)) = content else {
        return Vec::new();
    };
    listing
        .entries
        .iter()
        .map(|entry| entry.name.to_string())
        .collect()
}

/// Whether a node is open and its content isn't shown.
fn flagged(node: &DenNode) -> bool {
    matches!(
        node.access,
        DenAccess::Open {
            content_not_shown: true,
            ..
        }
    )
}

#[tokio::test]
async fn untitled_opens_to_its_four_entries() {
    let World { ports, .. } = test_world::world().await;

    let read = test_world::read(&ports, &alice(), false, &commission_path(untitled()), &[])
        .await
        .expect("alice opens Untitled");

    assert_eq!(read.node.name.to_string(), "Untitled");
    assert_eq!(read.crumbs, 0);
    assert_eq!(
        names(&read.content),
        ["attachments", "changelog", "products", "slots"]
    );
    let Some(DenContent::Listing(listing)) = &read.content else {
        unreachable!("the commission lists");
    };
    let kinds: Vec<(DenType, bool)> = listing
        .entries
        .iter()
        .map(|entry| {
            let file = matches!(
                entry.access,
                DenAccess::Open {
                    kind: NodeKind::File,
                    ..
                }
            );
            (entry.den_type, file)
        })
        .collect();
    let expected_kinds = [
        (DenType::CommissionAttachments, false),
        (DenType::CommissionChangelog, true),
        (DenType::CommissionProducts, false),
        (DenType::CommissionSlots, false),
    ];
    assert_eq!(kinds, expected_kinds);
}

#[tokio::test]
async fn attachments_list_the_files_by_name_each_flagged() {
    let World { ports, .. } = test_world::world().await;

    let read = test_world::read(
        &ports,
        &alice(),
        false,
        &commission_path(untitled()),
        &["attachments"],
    )
    .await
    .expect("alice opens the attachments");

    assert_eq!(names(&read.content), ["ref-abco.png", "sketch-01.png"]);
    let Some(DenContent::Listing(listing)) = &read.content else {
        unreachable!("attachments list");
    };
    assert!(listing.entries.iter().all(flagged));
    assert!(
        listing
            .entries
            .iter()
            .all(|entry| entry.den_type == DenType::File)
    );
}

#[tokio::test]
async fn a_file_opens_as_a_file() {
    let World { ports, .. } = test_world::world().await;
    let [ref_abco, _] = untitled_files();

    let read = test_world::read(
        &ports,
        &alice(),
        false,
        &commission_path(untitled()),
        &["attachments", &ref_abco.to_string()],
    )
    .await
    .expect("alice opens ref-abco.png");

    assert_eq!(read.node.name.to_string(), "ref-abco.png");
    assert_eq!(read.content, Some(DenContent::File));
}

#[tokio::test]
async fn slots_list_their_titles_and_each_opens_flagged_and_empty() {
    let World { ports, .. } = test_world::world().await;
    let root = commission_path(untitled());

    let read = test_world::read(&ports, &alice(), false, &root, &["slots"])
        .await
        .expect("alice opens the slots");

    assert_eq!(names(&read.content), ["Abco", "Ember", "Kael", "Open slot"]);
    for slot in untitled_slots() {
        let slot_key = slot.to_string();
        let opened = test_world::read(&ports, &alice(), false, &root, &["slots", &slot_key])
            .await
            .expect("alice opens a slot");

        assert!(flagged(&opened.node), "{slot_key}");
        let empty_listing = DenContent::Listing(Default::default());
        assert_eq!(opened.content, Some(empty_listing), "{slot_key}");
    }
}

#[tokio::test]
async fn products_are_empty_and_not_flagged() {
    let World { ports, .. } = test_world::world().await;

    let read = test_world::read(
        &ports,
        &alice(),
        false,
        &commission_path(untitled()),
        &["products"],
    )
    .await
    .expect("alice opens the products");

    assert!(!flagged(&read.node));
    assert_eq!(read.content, Some(DenContent::Listing(Default::default())));
}

#[tokio::test]
async fn products_hold_nothing_until_products_can_be_stored() {
    let World { ports, .. } = test_world::world().await;
    let alice = alice();
    let context = test_world::context(&ports, &alice);

    let products = children(&context, &CommissionPlace::Products(untitled()))
        .await
        .expect("no store fails");

    assert_eq!(products, Vec::new());
}

#[tokio::test]
async fn the_changelog_is_a_file() {
    let World { ports, .. } = test_world::world().await;

    let read = test_world::read(
        &ports,
        &alice(),
        false,
        &commission_path(untitled()),
        &["changelog"],
    )
    .await
    .expect("alice opens the changelog");

    assert_eq!(read.content, Some(DenContent::File));
    assert!(flagged(&read.node));
}

#[tokio::test]
async fn an_archived_commission_opens_by_its_path_marked_archived() {
    let World { ports, .. } = test_world::world().await;

    let read = test_world::read(&ports, &alice(), false, &commission_path(old_sketch()), &[])
        .await
        .expect("alice opens Old sketch by its path");

    assert_eq!(read.node.name.to_string(), "Old sketch");
    assert!(matches!(
        read.node.access,
        DenAccess::Open {
            soft_delete: Some(SoftDelete::Archived),
            own_level: Level::Private,
            ..
        }
    ));
}

#[tokio::test]
async fn a_commission_that_is_not_the_viewer_s_reads_as_one_that_does_not_exist() {
    let World { ports, .. } = test_world::world().await;
    let bob = bob();
    let nowhere = CommissionId::from(Uuid::from_u128(0x01a0_0000_0000_7000_8000_0000_0000_0099));

    let not_bobs = test_world::read(&ports, &bob, false, &commission_path(old_sketch()), &[]).await;
    let absent = test_world::read(&ports, &bob, false, &commission_path(nowhere), &[]).await;
    let context = test_world::context(&ports, &bob);
    let bobs_node = node(&context, old_sketch())
        .await
        .expect("no store fails")
        .expect("Old sketch exists");

    assert!(matches!(not_bobs, Err(DenError::NotFound)));
    assert!(matches!(absent, Err(DenError::NotFound)));
    assert_eq!(bobs_node.lifts, Vec::new());
    assert_eq!(bobs_node.level, Level::Private);
}

#[tokio::test]
async fn a_participant_holds_standing_at_the_commission() {
    let World { ports, .. } = test_world::world().await;
    let bob = bob();
    let context = test_world::context(&ports, &bob);

    let untitled_for_bob = node(&context, untitled())
        .await
        .expect("no store fails")
        .expect("Untitled exists");

    assert_eq!(untitled_for_bob.lifts, vec![STANDING]);
    assert_eq!(untitled_for_bob.state, NodeState::Live);
}

#[tokio::test]
async fn a_stored_title_the_name_rule_refuses_shows_unnamed() {
    let World { ports, backend, .. } = test_world::world().await;
    let alice = alice();
    let title = "Legacy photo"
        .parse::<CommissionTitle>()
        .expect("a valid test title");
    let seed = CommissionSeed::private(title, alice.clone());
    let legacy_id = seed.id;
    backend
        .seed_bound_commission(&seed)
        .await
        .expect("the legacy commission seeds");
    backend.seed_stored_title(legacy_id, "photo\u{202E}gpj.exe");

    let read = test_world::read(&ports, &alice, false, &commission_path(legacy_id), &[])
        .await
        .expect("alice opens her legacy commission");

    assert_eq!(read.node.name.to_string(), "Unnamed");
}

#[tokio::test]
async fn mounts_in_the_viewer_s_own_commissions_include_archived_ones() {
    let World { ports, .. } = test_world::world().await;
    let alice = alice();
    let context = test_world::context(&ports, &alice);

    let own = mounts(&context, &CommissionPlace::UserCommissions(alice.clone()))
        .await
        .expect("no store fails");
    let someone_else_s = mounts(&context, &CommissionPlace::UserCommissions(bob()))
        .await
        .expect("no store fails");

    let segments: Vec<String> = own.iter().map(|mount| mount.segment.to_string()).collect();
    let mut expected = vec![old_sketch().to_string(), untitled().to_string()];
    expected.sort();
    assert_eq!(segments, expected);
    assert!(
        own.iter()
            .all(|mount| mount.target.top_level() == Some(TopLevel::Commission))
    );
    assert_eq!(someone_else_s, Vec::new());
}

/// The fixture world with its commission and file stores recording every read.
async fn recorded() -> (World, CallLog) {
    let World {
        runtime,
        mut ports,
        backend,
    } = test_world::world().await;
    let log = CallLog::default();
    let commissions = RecordingCommissionStore::new(ports.commissions.clone(), log.clone());
    ports.commissions = Arc::new(commissions);
    ports.files = Arc::new(RecordingFileStore::new(ports.files.clone(), log.clone()));
    let world = World {
        runtime,
        ports,
        backend,
    };
    (world, log)
}

#[tokio::test]
async fn an_absent_commission_and_one_not_the_viewer_s_run_the_same_reads() {
    let (World { ports, .. }, log) = recorded().await;
    let bob = bob();
    let context = test_world::context(&ports, &bob);
    let nowhere = CommissionId::from(Uuid::from_u128(0x01a0_0000_0000_7000_8000_0000_0000_0098));

    node(&context, nowhere).await.expect("no store fails");
    let for_an_absent_commission = log.events();
    node(&context, old_sketch()).await.expect("no store fails");
    let for_both = log.events();

    let for_one_not_bobs = for_both[for_an_absent_commission.len()..].to_vec();
    assert_eq!(for_an_absent_commission, ["find_summary", "is_participant"]);
    assert_eq!(for_one_not_bobs, for_an_absent_commission);
}

#[tokio::test]
async fn a_participant_opening_their_commission_never_reads_the_whole_row() {
    let (World { ports, .. }, log) = recorded().await;
    let bob = bob();

    let read = test_world::read(&ports, &bob, false, &commission_path(untitled()), &[])
        .await
        .expect("bob opens Untitled");

    let reads = log.events();
    assert_eq!(read.node.name.to_string(), "Untitled");
    assert!(reads.contains(&"find_summary"), "{reads:?}");
    assert!(!reads.contains(&"find"), "{reads:?}");
}

#[tokio::test]
async fn an_absent_file_key_and_another_commission_s_run_the_same_reads() {
    let (World { ports, .. }, log) = recorded().await;
    let alice = alice();
    let context = test_world::context(&ports, &alice);
    let elsewhere = FileKey::from(Uuid::from_u128(0x01a0_0000_0000_7000_8000_0000_0000_0097));
    let stored_name = FileName::try_from("elsewhere.png").expect("a valid file name");
    let mut bytes: &[u8] = b"bytes of a file in no commission of alice's";
    ports
        .files
        .put(elsewhere, &stored_name, "image/png", &mut bytes)
        .await
        .expect("the file stores");
    let absent = segment(
        &FileKey::from(Uuid::from_u128(0x01a0_0000_0000_7000_8000_0000_0000_0096)).to_string(),
    );
    let attachments_place = CommissionPlace::Attachments(untitled());
    let before = log.events().len();

    let absent_child = child(&context, &attachments_place, &absent)
        .await
        .expect("no store fails");
    let middle = log.events().len();
    let elsewhere_child = child(
        &context,
        &attachments_place,
        &segment(&elsewhere.to_string()),
    )
    .await
    .expect("no store fails");

    let events = log.events();
    let for_an_absent_key = events[before..middle].to_vec();
    let for_another_commission_s_key = events[middle..].to_vec();
    assert_eq!(absent_child, None);
    assert_eq!(elsewhere_child, None);
    assert_eq!(for_an_absent_key, ["files"]);
    assert_eq!(for_another_commission_s_key, for_an_absent_key);
}

#[test]
fn the_verdict_answers_each_standing_by_the_act_s_need() {
    let at = |role| CommissionStanding {
        role,
        level: Level::Listed,
        archived: true,
    };
    let granted = |role| {
        Verdict::Granted(StandingFacts {
            role,
            level: Level::Listed,
            archived: true,
        })
    };
    let cases = [
        (None, CommissionRole::Participant, Verdict::NotFound),
        (
            Some(at(None)),
            CommissionRole::Participant,
            Verdict::NotFound,
        ),
        (
            Some(at(Some(CommissionRole::Participant))),
            CommissionRole::Owner,
            Verdict::Forbidden,
        ),
        (
            Some(at(Some(CommissionRole::Participant))),
            CommissionRole::Participant,
            granted(CommissionRole::Participant),
        ),
        (
            Some(at(Some(CommissionRole::Owner))),
            CommissionRole::Participant,
            granted(CommissionRole::Owner),
        ),
        (
            Some(at(Some(CommissionRole::Owner))),
            CommissionRole::Owner,
            granted(CommissionRole::Owner),
        ),
    ];

    for (standing, needs, expected) in cases {
        assert_eq!(verdict(standing, needs), expected, "{standing:?} {needs:?}");
    }
}
