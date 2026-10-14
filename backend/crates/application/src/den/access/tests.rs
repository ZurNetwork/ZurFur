use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use adapter_mem::fixture::{alice, bob, old_sketch, supreme_arts, untitled, untitled_files};
use domain::{
    elements::{
        commission::{Commission, CommissionFile, CommissionTitle, FileName, Visibility},
        did::Did,
        profile::Profile,
        user::User,
    },
    ports::{ProfileSource, UnitOfWork, UserStore},
};
use test_support::recording::{CallLog, RecordingCommissionStore, RecordingFileStore};
use tokio::time::Instant;
use uuid::Uuid;

use super::*;
use crate::{
    den::names::ACCOUNTS,
    test_world::{self, World, path, segment},
};

/// A commission id nothing is stored under.
fn nowhere() -> CommissionId {
    CommissionId::from(Uuid::from_u128(0x01a0_0000_0000_7000_8000_0000_0000_00a1))
}

/// A user who takes part in nothing in the fixture world.
fn stranger() -> UserId {
    let did = Did::from("did:plc:den-access-stranger".to_owned());
    UserId::from(did)
}

/// Whether the check answers "opens".
fn opened(answer: &Result<Opens, DenError>) -> bool {
    answer.is_ok()
}

/// Whether the check answers the one not-found.
fn not_found(answer: &Result<Opens, DenError>) -> bool {
    matches!(answer, Err(DenError::NotFound))
}

/// Seeds a commission of alice's at `visibility`, which no one else takes part in.
async fn seed_alice_s(world: &World, title: &str, visibility: Visibility) -> CommissionId {
    let title = title
        .parse::<CommissionTitle>()
        .expect("a valid test title");
    let mut commission = Commission::create(title, alice(), chrono::Utc::now(), None);
    commission.visibility = visibility;
    let id = commission.id;
    world
        .runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.commissions().create(&commission).await
        })
        .await
        .expect("the commission seeds");
    id
}

/// Seeds a file into "Old sketch", whose key is no file of "Untitled".
async fn seed_old_sketch_file(world: &World) -> FileKey {
    let key = FileKey::from(Uuid::from_u128(0x01a0_0000_0000_7000_8000_0000_0000_00a2));
    let name = FileName::try_from("old.png").expect("a valid file name");
    let mut bytes: &[u8] = b"bytes of a file in Old sketch";
    world
        .ports
        .files
        .put(key, &name, "image/png", &mut bytes)
        .await
        .expect("the file stores");
    let file = CommissionFile {
        id: key,
        commission_id: old_sketch(),
        uploaded_by: alice(),
        created_at: chrono::Utc::now(),
    };
    world
        .runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| uow.commissions().add_file(&file).await)
        .await
        .expect("the file links to Old sketch");
    key
}

/// Whether `viewer` opens `node` in `world`.
async fn check(world: &World, viewer: &UserId, node: ServedNode) -> Result<Opens, DenError> {
    may_open(&world.ports, viewer, node).await
}

#[tokio::test]
async fn the_viewer_s_own_root_opens() {
    let world = test_world::world().await;

    for viewer in [alice(), bob()] {
        let answer = check(&world, &viewer, ServedNode::OwnRoot).await;

        assert!(opened(&answer), "{viewer} opens their own root");
    }
}

#[tokio::test]
async fn an_account_opens_for_its_member_only() {
    let world = test_world::world().await;
    let absent_did = Did::from("did:plc:den-access-no-account".to_owned());
    let absent = AccountId::from(absent_did);

    let for_alice = check(&world, &alice(), ServedNode::Account(supreme_arts())).await;
    let for_bob = check(&world, &bob(), ServedNode::Account(supreme_arts())).await;
    let for_absent = check(&world, &alice(), ServedNode::Account(absent)).await;

    assert!(opened(&for_alice));
    assert!(not_found(&for_bob));
    assert!(not_found(&for_absent));
}

#[tokio::test]
async fn untitled_and_its_changelog_open_for_its_participants() {
    let world = test_world::world().await;

    for participant in [alice(), bob()] {
        let commission = check(&world, &participant, ServedNode::Commission(untitled())).await;
        let changelog = check(&world, &participant, ServedNode::Changelog(untitled())).await;

        assert!(opened(&commission), "{participant} opens Untitled");
        assert!(opened(&changelog), "{participant} opens its changelog");
    }
}

#[tokio::test]
async fn a_stranger_is_refused_untitled_as_if_it_were_absent() {
    let world = test_world::world().await;
    let stranger = stranger();

    let not_theirs = check(&world, &stranger, ServedNode::Commission(untitled())).await;
    let absent = check(&world, &stranger, ServedNode::Commission(nowhere())).await;
    let not_their_changelog = check(&world, &stranger, ServedNode::Changelog(untitled())).await;
    let absent_changelog = check(&world, &stranger, ServedNode::Changelog(nowhere())).await;

    assert!(not_found(&not_theirs));
    assert!(not_found(&absent));
    assert!(not_found(&not_their_changelog));
    assert!(not_found(&absent_changelog));
}

#[tokio::test]
async fn a_file_of_untitled_opens_for_its_participants() {
    let world = test_world::world().await;
    let [ref_abco, sketch_01] = untitled_files();

    for participant in [alice(), bob()] {
        for file in [ref_abco, sketch_01] {
            let answer = check(&world, &participant, ServedNode::File(untitled(), file)).await;

            assert!(opened(&answer), "{participant} opens {file}");
        }
    }
}

#[tokio::test]
async fn an_absent_file_key_and_another_commission_s_are_not_found_under_untitled() {
    let world = test_world::world().await;
    let elsewhere = seed_old_sketch_file(&world).await;
    let absent = FileKey::from(Uuid::from_u128(0x01a0_0000_0000_7000_8000_0000_0000_00a3));
    let alice = alice();

    let absent_key = check(&world, &alice, ServedNode::File(untitled(), absent)).await;
    let elsewhere_key = check(&world, &alice, ServedNode::File(untitled(), elsewhere)).await;
    let at_home = check(&world, &alice, ServedNode::File(old_sketch(), elsewhere)).await;

    assert!(not_found(&absent_key));
    assert!(not_found(&elsewhere_key));
    assert!(opened(&at_home), "the file opens under its own commission");
}

#[tokio::test]
async fn a_stranger_is_refused_a_file_of_untitled() {
    let world = test_world::world().await;
    let [ref_abco, _] = untitled_files();

    let answer = check(&world, &stranger(), ServedNode::File(untitled(), ref_abco)).await;

    assert!(not_found(&answer));
}

#[tokio::test]
async fn an_archived_commission_s_changelog_opens_by_its_typed_path_only_with_standing() {
    let world = test_world::world().await;

    let for_alice = check(&world, &alice(), ServedNode::Changelog(old_sketch())).await;
    let for_bob = check(&world, &bob(), ServedNode::Changelog(old_sketch())).await;

    assert!(opened(&for_alice));
    assert!(not_found(&for_bob));
}

#[tokio::test]
async fn a_card_does_not_count_as_opening() {
    let world = test_world::world().await;
    let listed = seed_alice_s(&world, "Listed piece", Visibility::Listed).await;
    let stranger = stranger();

    let card = check(&world, &stranger, ServedNode::Commission(listed)).await;
    let card_changelog = check(&world, &stranger, ServedNode::Changelog(listed)).await;
    let for_alice = check(&world, &alice(), ServedNode::Commission(listed)).await;

    assert!(not_found(&card));
    assert!(not_found(&card_changelog));
    assert!(opened(&for_alice));
}

#[tokio::test]
async fn a_public_commission_opens_for_a_stranger_but_its_private_changelog_does_not() {
    let world = test_world::world().await;
    let public = seed_alice_s(&world, "Public piece", Visibility::Public).await;
    let stranger = stranger();

    let commission = check(&world, &stranger, ServedNode::Commission(public)).await;
    let changelog = check(&world, &stranger, ServedNode::Changelog(public)).await;

    assert!(opened(&commission));
    assert!(not_found(&changelog));
}

/// The fixture world with its commission and file stores recording every read.
async fn recorded() -> (World, CallLog) {
    let World { runtime, mut ports } = test_world::world().await;
    let log = CallLog::default();
    let commissions = RecordingCommissionStore::new(ports.commissions.clone(), log.clone());
    ports.commissions = Arc::new(commissions);
    ports.files = Arc::new(RecordingFileStore::new(ports.files.clone(), log.clone()));
    (World { runtime, ports }, log)
}

/// The reads the check runs for `viewer` and `node`, alone.
async fn reads_for(
    world: &World,
    log: &CallLog,
    viewer: &UserId,
    node: ServedNode,
) -> Vec<&'static str> {
    let before = log.events().len();
    let _answer = check(world, viewer, node).await;
    log.events()[before..].to_vec()
}

#[tokio::test]
async fn a_stranger_s_absent_and_not_theirs_changelog_run_the_same_reads() {
    let (world, log) = recorded().await;
    let stranger = stranger();

    let for_absent = reads_for(&world, &log, &stranger, ServedNode::Changelog(nowhere())).await;
    let for_not_theirs =
        reads_for(&world, &log, &stranger, ServedNode::Changelog(untitled())).await;

    assert_eq!(for_absent, ["find", "is_participant"]);
    assert_eq!(for_not_theirs, for_absent);
}

#[tokio::test]
async fn an_absent_and_another_commission_s_file_key_run_the_same_reads() {
    let (world, log) = recorded().await;
    let elsewhere = seed_old_sketch_file(&world).await;
    let absent = FileKey::from(Uuid::from_u128(0x01a0_0000_0000_7000_8000_0000_0000_00a4));
    let bob = bob();

    let for_absent = reads_for(&world, &log, &bob, ServedNode::File(untitled(), absent)).await;
    let for_elsewhere =
        reads_for(&world, &log, &bob, ServedNode::File(untitled(), elsewhere)).await;

    assert_eq!(for_absent, ["find", "is_participant", "files"]);
    assert_eq!(for_elsewhere, for_absent);
}

#[tokio::test]
async fn the_proof_carries_the_node_it_was_checked_for() {
    let world = test_world::world().await;
    let [ref_abco, _] = untitled_files();
    let file = ServedNode::File(untitled(), ref_abco);

    let opens = check(&world, &alice(), file.clone())
        .await
        .expect("alice opens a file of Untitled");

    assert_eq!(opens.node(), &file);
}

#[tokio::test]
async fn a_node_reached_through_a_mount_does_not_count_as_opening() {
    let world = test_world::world().await;
    let alice = alice();
    let anchor = path(&format!("/user/{alice}"));
    let account = supreme_arts();
    let steps = [segment(ACCOUNTS), segment(account.as_ref())];
    let host = DenHost::new(&world.ports, &alice, false);
    let reader = Reader::over(&host, Caps::default());
    let resolved = reader
        .walk(&anchor, &steps)
        .await
        .expect("alice walks to Supreme Arts through her accounts");

    let answer = opens_at(&world.ports, &alice, &anchor, &steps).await;

    let landed_at_the_account = path(&format!("/account/{account}"));
    assert!(matches!(
        &resolved.node().view,
        View::Open { real_path, .. } if real_path == &landed_at_the_account
    ));
    assert!(matches!(answer, Err(DenError::NotFound)));
}

/// A user store whose every read fails.
struct FailingUsers;

#[async_trait::async_trait]
impl UserStore for FailingUsers {
    async fn find(&self, _id: &UserId) -> anyhow::Result<Option<User>> {
        Err(anyhow::anyhow!("the user store is down"))
    }

    async fn find_by_did(&self, _did: &Did) -> anyhow::Result<Option<User>> {
        Err(anyhow::anyhow!("the user store is down"))
    }
}

#[tokio::test]
async fn a_store_failure_stays_a_store_failure() {
    let World { mut ports, .. } = test_world::world().await;
    ports.users = Arc::new(FailingUsers);

    let answer = may_open(&ports, &alice(), ServedNode::OwnRoot).await;

    assert!(matches!(answer, Err(DenError::Store(_))));
}

/// A PDS that never answers, counting every fetch it is asked for.
#[derive(Default)]
struct SilentPds {
    fetches: AtomicUsize,
}

impl SilentPds {
    fn fetches(&self) -> usize {
        self.fetches.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl ProfileSource for SilentPds {
    async fn fetch(&self, _did: &Did) -> anyhow::Result<Profile> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        std::future::pending().await
    }
}

#[tokio::test(start_paused = true)]
async fn the_check_names_a_cold_root_without_starting_or_waiting_on_a_lookup() {
    let World { mut ports, runtime } = test_world::world().await;
    let carol = Did::from("did:plc:den-access-carol".to_owned());
    let provisioned = carol.clone();
    runtime
        .transaction(async move |uow: &mut dyn UnitOfWork| {
            uow.users().provision(&provisioned).await
        })
        .await
        .expect("carol signs up");
    let pds = Arc::new(SilentPds::default());
    ports.profile_source = pds.clone();
    let carol = UserId::from(carol);
    let started = Instant::now();

    let opens = may_open(&ports, &carol, ServedNode::OwnRoot)
        .await
        .expect("carol opens her own root");

    let waited = started.elapsed();
    for _ in 0..8 {
        tokio::task::yield_now().await;
    }
    assert_eq!(opens.node(), &ServedNode::OwnRoot);
    assert_eq!(waited, Duration::ZERO, "the check never waits on a name");
    assert_eq!(pds.fetches(), 0, "the check never starts a lookup");
}
