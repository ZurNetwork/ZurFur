use std::sync::Arc;

use adapter_mem::fixture::alice;
use domain::elements::user::UserId;
use test_support::recording::{
    CallLog, RecordingAccountStore, RecordingCharacterStore, RecordingCommissionStore,
    RecordingProfileCache, RecordingProfileSource, RecordingUserStore, SubjectLog,
};

use super::*;
use crate::{
    Ports,
    den::{DenAccess, DenContent, DenError, DenListing, NodeKind, NodeLevel},
    test_world::{self, World, path, segment},
};

/// A DID no user has in the fixture world.
const NO_USER_DID: &str = "did:plc:nouserhasthisdidaaaaaaaa";

/// A DID no user has.
fn no_user() -> Did {
    Did::from(NO_USER_DID.to_owned())
}

/// The real path of `poster`'s posts.
fn posts_of(poster: &Did) -> atta::RealPath {
    path(&format!("post/{poster}"))
}

/// What the Den shows for the stub, as text: everything a driver can render from it.
async fn stub_answer(ports: &Ports, viewer: &UserId, poster: &Did) -> String {
    let read = test_world::read(ports, viewer, false, &posts_of(poster), &[])
        .await
        .expect("the stub opens for any DID");
    format!("{read:?}")
}

#[tokio::test]
async fn the_stub_answers_alice_and_a_did_with_no_user_the_same_bytes_and_not_found_below() {
    let World { ports, .. } = test_world::world().await;
    let alice = alice();

    let alice_s = stub_answer(&ports, &alice, alice.did()).await;
    let no_user_s = stub_answer(&ports, &alice, &no_user()).await;
    let below_alice_s =
        test_world::read(&ports, &alice, false, &posts_of(alice.did()), &["anything"]).await;
    let below_no_user_s =
        test_world::read(&ports, &alice, false, &posts_of(&no_user()), &["anything"]).await;

    assert_eq!(alice_s, no_user_s);
    assert!(!alice_s.contains(alice.did().as_ref()), "{alice_s}");
    assert!(matches!(below_alice_s, Err(DenError::NotFound)));
    assert!(matches!(below_no_user_s, Err(DenError::NotFound)));
}

#[tokio::test]
async fn the_stub_opens_named_posts_public_with_its_content_not_shown_and_an_empty_page() {
    let World { ports, .. } = test_world::world().await;
    let alice = alice();

    let read = test_world::read(&ports, &alice, false, &posts_of(alice.did()), &[])
        .await
        .expect("alice's posts open");

    let expected_access = DenAccess::Open {
        kind: NodeKind::Directory,
        own_level: NodeLevel::Public,
        content_not_shown: true,
        soft_delete: None,
    };
    assert_eq!(read.node.name.to_string(), POSTS);
    assert_eq!(read.node.den_type, DenType::Post);
    assert_eq!(read.node.access, expected_access);
    assert_eq!(
        read.content,
        Some(DenContent::Listing(DenListing::default()))
    );
}

/// The fixture world's ports with every store an arm could read recording its calls.
fn recording_ports(world: &mut World, log: &CallLog) -> Ports {
    let subjects = SubjectLog::default();
    let runtime = &mut world.runtime;
    runtime.users = Arc::new(RecordingUserStore::new(
        runtime.users.clone(),
        log.clone(),
        subjects.clone(),
    ));
    runtime.accounts = Arc::new(RecordingAccountStore::new(
        runtime.accounts.clone(),
        log.clone(),
        subjects.clone(),
    ));
    runtime.characters = Arc::new(RecordingCharacterStore::new(
        runtime.characters.clone(),
        log.clone(),
        subjects.clone(),
    ));
    runtime.commissions = Arc::new(RecordingCommissionStore::new(
        runtime.commissions.clone(),
        log.clone(),
    ));
    runtime.profile_cache = Arc::new(RecordingProfileCache::new(
        runtime.profile_cache.clone(),
        log.clone(),
        subjects.clone(),
    ));
    runtime.profile_source = Arc::new(RecordingProfileSource::new(
        runtime.profile_source.clone(),
        log.clone(),
        subjects,
    ));
    Ports::from(&world.runtime)
}

#[tokio::test]
async fn the_arm_answers_any_did_and_holds_nothing_without_a_store_call() {
    let mut world = test_world::world().await;
    let log = CallLog::default();
    let ports = recording_ports(&mut world, &log);
    let alice = alice();
    let context = test_world::context(&ports, &alice);

    for poster in [alice.did().clone(), no_user()] {
        let place = PostPlace::Posts(poster.clone());

        let answer = node(&context, &poster).await.expect("no store is read");
        let child = child(&context, &place, &segment("anything"))
            .await
            .expect("no store is read");
        let children = children(&context, &place).await.expect("no store is read");
        let mounts = mounts(&context, &place).await.expect("no store is read");

        let expected = Node {
            place: Place::Post(place),
            name: names::fixed_name(POSTS).expect("a valid fixed word"),
            level: Level::Public,
            lifts: Vec::new(),
            state: NodeState::Live,
        };
        assert_eq!(answer, Some(expected), "{poster}");
        assert_eq!(child, None, "{poster}");
        assert_eq!(children, Vec::new(), "{poster}");
        assert_eq!(mounts, Vec::new(), "{poster}");
    }
    assert_eq!(log.events(), Vec::<&str>::new());
}
