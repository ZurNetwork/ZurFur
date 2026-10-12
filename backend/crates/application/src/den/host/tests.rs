use domain::elements::did::Did;

use super::*;

/// A bag over the in-memory runtime's ports, built as this crate's own [`Ports`].
fn ports() -> Ports {
    let did = Did::from("did:plc:den-host-test".to_owned());
    let runtime = test_support::runtime::mem(&did).build().runtime;
    Ports {
        database: runtime.database.clone(),
        users: runtime.users.clone(),
        accounts: runtime.accounts.clone(),
        commissions: runtime.commissions.clone(),
        changelog: runtime.changelog.clone(),
        profile_source: runtime.profile_source.clone(),
        profile_cache: runtime.profile_cache.clone(),
        identity_resolver: runtime.identity_resolver.clone(),
        did_minter: runtime.did_minter.clone(),
        files: runtime.files.clone(),
        workflows: runtime.workflows.clone(),
        columns: runtime.columns.clone(),
        characters: runtime.characters.clone(),
    }
}

fn alice() -> UserId {
    UserId::from(Did::from("did:plc:alice".to_owned()))
}

fn segment(text: &str) -> Segment {
    Segment::try_from(text).expect("a valid test segment")
}

fn path(texts: &[&str]) -> RealPath {
    texts.iter().map(|text| segment(text)).collect()
}

fn open_by_standing() -> Access {
    Access {
        level: Level::Public,
        via: atta::Via::Standing,
    }
}

#[tokio::test]
async fn another_viewer_is_answered_nothing_by_every_method() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, true);
    let bob = DenHost::viewer_of(&UserId::from(Did::from("did:plc:bob".to_owned())));
    let root = RealPath::default();
    let user_directory = path(&["user"]);
    let recorded = host
        .child(host.viewer(), &root, &segment("user"))
        .await
        .expect("no store is read");
    assert!(recorded.is_some(), "the session's viewer is answered");

    let child = host
        .child(&bob, &root, &segment("user"))
        .await
        .expect("no store is read");
    let children = host.children(&bob, &root).await.expect("no store is read");
    let mounts = host
        .mounts(&bob, &user_directory)
        .await
        .expect("no store is read");
    let admission = host.admit(
        &bob,
        &user_directory,
        &recorded.expect("answered above").metadata,
        open_by_standing(),
        Reach::Path,
    );

    assert_eq!(child, None);
    assert_eq!(children, Vec::new());
    assert_eq!(mounts, Vec::new());
    assert_eq!(admission, Admission::Refuse);
}

#[tokio::test]
async fn a_folder_the_host_never_answered_for_holds_nothing() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let viewer = host.viewer().clone();
    let unrecorded = path(&["user"]);

    let child = host
        .child(&viewer, &unrecorded, &segment("did:plc:alice"))
        .await
        .expect("no store is read");
    let children = host
        .children(&viewer, &unrecorded)
        .await
        .expect("no store is read");
    let mounts = host
        .mounts(&viewer, &unrecorded)
        .await
        .expect("no store is read");

    assert_eq!(child, None);
    assert_eq!(children, Vec::new());
    assert_eq!(mounts, Vec::new());
}

#[tokio::test]
async fn the_top_level_directories_are_public_directories_that_list_and_mount_nothing() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let viewer = host.viewer().clone();
    let root = RealPath::default();

    for word in ["user", "account", "character", "commission", "post"] {
        let found = host
            .child(&viewer, &root, &segment(word))
            .await
            .expect("no store is read")
            .expect("a top-level directory");
        let directory = path(&[word]);
        let children = host
            .children(&viewer, &directory)
            .await
            .expect("no store is read");
        let mounts = host
            .mounts(&viewer, &directory)
            .await
            .expect("no store is read");

        let metadata = found.metadata;
        assert_eq!(metadata.key, segment(word));
        assert_eq!(metadata.parent, Some(root.clone()));
        assert_eq!(metadata.level, Level::Public);
        assert_eq!(metadata.name.as_ref(), word);
        assert_eq!(host.kind(&metadata.typ), Kind::Directory, "{word}");
        assert_eq!(found.lifts, Vec::new());
        assert_eq!(children, Vec::new(), "{word}");
        assert_eq!(mounts, Vec::new(), "{word}");
    }
}

#[tokio::test]
async fn the_global_root_holds_the_top_level_directories_and_no_mounts() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let viewer = host.viewer().clone();
    let root = RealPath::default();

    let children = host
        .children(&viewer, &root)
        .await
        .expect("no store is read");
    let mounts = host.mounts(&viewer, &root).await.expect("no store is read");

    let keys: Vec<String> = children
        .iter()
        .map(|found| found.metadata.key.to_string())
        .collect();
    assert_eq!(keys, ["user", "account", "character", "commission", "post"]);
    assert_eq!(mounts, Vec::new());
}

#[tokio::test]
async fn a_name_under_root_that_is_no_top_level_directory_is_absent() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let viewer = host.viewer().clone();

    let blob = host
        .child(&viewer, &RealPath::default(), &segment("blob"))
        .await
        .expect("no store is read");

    assert_eq!(blob, None);
}

#[tokio::test]
async fn a_key_that_is_not_its_directory_s_id_is_absent() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let viewer = host.viewer().clone();
    let root = RealPath::default();
    for (top_level, key) in [("user", "not-a-did"), ("commission", "not-a-uuid")] {
        host.child(&viewer, &root, &segment(top_level))
            .await
            .expect("no store is read");

        let child = host
            .child(&viewer, &path(&[top_level]), &segment(key))
            .await
            .expect("no store is read");

        assert_eq!(child, None, "{top_level}/{key}");
    }
}

#[tokio::test]
async fn an_unknown_type_key_is_a_file() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);

    let unknown = host.kind(&Type::from("commission.participants".to_owned()));
    let changelog = host.kind(&Type::from("commission.changelog".to_owned()));
    let user = host.kind(&Type::from("user".to_owned()));

    assert_eq!(unknown, Kind::File);
    assert_eq!(changelog, Kind::File);
    assert_eq!(user, Kind::Directory);
}

#[tokio::test]
async fn admit_answers_from_the_record() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let viewer = host.viewer().clone();
    let root = RealPath::default();
    let found = host
        .child(&viewer, &root, &segment("commission"))
        .await
        .expect("no store is read")
        .expect("a top-level directory");

    let recorded = host.admit(
        &viewer,
        &path(&["commission"]),
        &found.metadata,
        open_by_standing(),
        Reach::Listing,
    );
    let unrecorded = host.admit(
        &viewer,
        &path(&["post"]),
        &found.metadata,
        open_by_standing(),
        Reach::Listing,
    );

    assert_eq!(recorded, Admission::Admit);
    assert_eq!(unrecorded, Admission::Refuse);
    assert_eq!(host.soft_delete(&path(&["commission"])), Some(None));
    assert_eq!(host.soft_delete(&path(&["post"])), None);
}
