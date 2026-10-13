use std::sync::Arc;

use adapter_mem::fixture::{alice, bob, seed_identity};
use test_support::recording::{
    CallLog, RecordingAccountStore, RecordingCharacterStore, RecordingProfileCache,
    RecordingProfileSource, RecordingUserStore, SubjectLog,
};

use super::*;
use crate::{App, Ports};

/// The fixture world, every identity read port recording its calls and whose data each
/// call asked for.
struct Recorded {
    app: App,
    log: CallLog,
    subjects: SubjectLog,
}

async fn recorded() -> Recorded {
    let mut mem = test_support::runtime::mem(alice().did()).build();
    seed_identity(&mem.backend)
        .await
        .expect("seeds identity's fixtures");
    let log = CallLog::default();
    let subjects = SubjectLog::default();
    let runtime = &mut mem.runtime;
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
    runtime.profile_cache = Arc::new(RecordingProfileCache::new(
        runtime.profile_cache.clone(),
        log.clone(),
        subjects.clone(),
    ));
    runtime.profile_source = Arc::new(RecordingProfileSource::new(
        runtime.profile_source.clone(),
        log.clone(),
        subjects.clone(),
    ));
    Recorded {
        app: App::new(Ports::from(&mem.runtime)),
        log,
        subjects,
    }
}

fn segment(text: &str) -> Segment {
    Segment::try_from(text).expect("a valid test segment")
}

#[tokio::test]
async fn another_user_s_directory_is_absent_before_any_store_is_read() {
    let recorded = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: recorded.app.ports(),
        viewer: &alice,
    };

    let node = node(&context, &bob()).await.expect("no store fails");

    assert_eq!(node, None);
    assert_eq!(recorded.log.events(), Vec::<&str>::new());
}

#[tokio::test]
async fn another_user_s_folders_hold_nothing_and_read_no_store() {
    let recorded = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: recorded.app.ports(),
        viewer: &alice,
    };
    let places = [
        UserPlace::User(bob()),
        UserPlace::Accounts(bob()),
        UserPlace::Characters(bob()),
    ];

    for place in places {
        let children = children(&context, &place).await.expect("no store fails");
        let mounts = mounts(&context, &place).await.expect("no store fails");
        let child = child(&context, &place, &segment(ACCOUNTS))
            .await
            .expect("no store fails");

        assert_eq!(children, Vec::new(), "{place:?}");
        assert_eq!(mounts, Vec::new(), "{place:?}");
        assert_eq!(child, None, "{place:?}");
    }
    assert_eq!(recorded.log.events(), Vec::<&str>::new());
}

#[tokio::test]
async fn the_viewer_s_root_is_private_lifted_by_standing_and_live() {
    let recorded = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: recorded.app.ports(),
        viewer: &alice,
    };

    let root = node(&context, &alice)
        .await
        .expect("no store fails")
        .expect("alice's root");

    let expected = Node {
        place: Place::User(UserPlace::User(alice.clone())),
        name: NodeName::try_from("Alice").expect("a valid name"),
        level: Level::Private,
        lifts: vec![STANDING],
        state: NodeState::Live,
    };
    assert_eq!(root, expected);
    assert_eq!(recorded.log.events(), ["find_user", "get_profile"]);
}

#[tokio::test]
async fn the_viewer_s_root_holds_three_system_folders_found_by_key_without_a_store() {
    let recorded = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: recorded.app.ports(),
        viewer: &alice,
    };
    let root = UserPlace::User(alice.clone());

    let folders = children(&context, &root).await.expect("no store fails");
    let commissions = child(&context, &root, &segment(COMMISSIONS))
        .await
        .expect("no store fails");
    let unknown = child(&context, &root, &segment("posts"))
        .await
        .expect("no store fails");

    let places: Vec<Place> = folders.iter().map(|folder| folder.place.clone()).collect();
    let expected_places = [
        Place::User(UserPlace::Accounts(alice.clone())),
        Place::User(UserPlace::Characters(alice.clone())),
        Place::Commission(CommissionPlace::UserCommissions(alice.clone())),
    ];
    assert_eq!(places, expected_places);
    for folder in &folders {
        assert_eq!(folder.level, Level::Private);
        assert_eq!(folder.lifts, Vec::new());
        assert_eq!(folder.state, NodeState::Live);
    }
    assert_eq!(
        commissions.map(|folder| folder.place),
        Some(expected_places[2].clone())
    );
    assert_eq!(unknown, None);
    assert_eq!(recorded.log.events(), Vec::<&str>::new());
}

#[tokio::test]
async fn the_viewer_s_root_mounts_their_posts_without_a_store() {
    let recorded = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: recorded.app.ports(),
        viewer: &alice,
    };

    let mounts = mounts(&context, &UserPlace::User(alice.clone()))
        .await
        .expect("no store fails");

    let posts = DenMount {
        segment: segment(POSTS),
        target: Place::Post(PostPlace::Posts(alice.did().clone())),
    };
    assert_eq!(mounts, [posts]);
    assert_eq!(recorded.log.events(), Vec::<&str>::new());
}

#[tokio::test]
async fn the_viewer_s_mounts_are_read_with_their_own_id_only() {
    let recorded = recorded().await;
    let alice = alice();
    let context = DenContext {
        ports: recorded.app.ports(),
        viewer: &alice,
    };

    let accounts = mounts(&context, &UserPlace::Accounts(alice.clone()))
        .await
        .expect("no store fails");
    let characters = mounts(&context, &UserPlace::Characters(alice.clone()))
        .await
        .expect("no store fails");

    let alice_did = alice.to_string();
    assert_eq!(accounts.len(), 1);
    assert_eq!(characters.len(), 2);
    assert_eq!(
        recorded.log.events(),
        [
            "list_for_user",
            "list_deactivated_for_owner",
            "list_kept_by"
        ]
    );
    assert_eq!(
        recorded.subjects.subjects(),
        [alice_did.clone(), alice_did.clone(), alice_did]
    );
}

#[tokio::test]
async fn a_user_with_no_row_has_no_directory() {
    let recorded = recorded().await;
    let carol = UserId::from(domain::elements::did::Did::from(
        "did:plc:carol-den".to_owned(),
    ));
    let context = DenContext {
        ports: recorded.app.ports(),
        viewer: &carol,
    };

    let node = node(&context, &carol).await.expect("no store fails");

    assert_eq!(node, None);
    assert_eq!(recorded.log.events(), ["find_user"]);
}

/// A profile cache that answers bob's profile whatever DID it is asked for.
struct MisfiledProfileCache;

#[async_trait::async_trait]
impl domain::ports::ProfileCache for MisfiledProfileCache {
    async fn get(
        &self,
        _did: &domain::elements::did::Did,
    ) -> anyhow::Result<Option<domain::elements::profile::Profile>> {
        let bob_s = domain::elements::profile::Profile::new(bob().did().clone(), "bob.test")
            .with_display_name("Bob");
        Ok(Some(bob_s))
    }

    async fn put(&self, _profile: &domain::elements::profile::Profile) -> anyhow::Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn a_cached_profile_for_another_did_never_names_the_root() {
    let mut mem = test_support::runtime::mem(alice().did()).build();
    seed_identity(&mem.backend)
        .await
        .expect("seeds identity's fixtures");
    mem.runtime.profile_cache = Arc::new(MisfiledProfileCache);
    let app = App::new(Ports::from(&mem.runtime));
    let alice = alice();
    let context = DenContext {
        ports: app.ports(),
        viewer: &alice,
    };

    let root = node(&context, &alice)
        .await
        .expect("no store fails")
        .expect("alice's root");

    assert_eq!(root.name.as_ref(), alice.to_string());
}
