use domain::elements::{commission::CommissionId, did::Did, name::NodeName};
use uuid::Uuid;

use super::*;
use crate::{
    den::{DenThing, SoftDelete},
    test_world::World,
};

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
        profile_lookups: crate::user::ProfileLookups::default(),
        identity_resolver: runtime.identity_resolver.clone(),
        did_minter: runtime.did_minter.clone(),
        files: runtime.files.clone(),
        workflows: runtime.workflows.clone(),
        columns: runtime.columns.clone(),
        characters: runtime.characters.clone(),
        bug_reports: runtime.bug_reports.clone(),
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
        level: atta::Level::Public,
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
    let unrecorded = path(&["nowhere"]);

    let child = host
        .child(&viewer, &unrecorded, &segment("user"))
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
        assert_eq!(metadata.level, atta::Level::Public);
        assert_eq!(metadata.name.as_ref(), word);
        assert_eq!(host.kind(&metadata.typ), Kind::Directory, "{word}");
        assert_eq!(found.lifts, Vec::new());
        assert_eq!(children, Vec::new(), "{word}");
        assert_eq!(mounts, Vec::new(), "{word}");
    }
}

#[tokio::test]
async fn the_global_root_is_traverse_only_and_lists_and_mounts_nothing() {
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

    assert_eq!(children, Vec::new());
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

    let unknown = host.kind(&Type::from("not a den type".to_owned()));
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
    assert_eq!(
        host.recorded(&path(&["commission"]))
            .map(|facts| facts.soft_delete),
        Some(None)
    );
    assert_eq!(
        host.recorded(&path(&["post"]))
            .map(|facts| facts.soft_delete),
        None
    );
}

#[tokio::test]
async fn only_the_session_s_own_user_is_looked_up() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);

    let own = host.own_user(&segment("did:plc:alice"));
    let other = host.own_user(&segment("did:plc:bob"));
    let malformed = host.own_user(&segment("alice"));

    assert_eq!(own, Some(alice.clone()));
    assert_eq!(other, None);
    assert_eq!(malformed, None);
}

/// Records `place` at `at` as if an arm had answered for it, live.
fn record(host: &DenHost<'_>, at: &RealPath, place: Place) {
    let recorded = Recorded {
        place,
        state: NodeState::Live,
    };
    host.record().insert(at.clone(), recorded);
}

fn commission_id(bits: u128) -> CommissionId {
    CommissionId::from(Uuid::from_u128(bits))
}

#[tokio::test]
async fn a_folder_whose_content_is_not_shown_is_closed_to_typed_paths() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let posts = path(&["post", "did:plc:alice"]);
    let attachments = path(&["commission", "c1", "attachments"]);
    let poster = Did::from("did:plc:alice".to_owned());
    record(&host, &posts, Place::Post(post::PostPlace::Posts(poster)));
    let attachments_place = commission::den::CommissionPlace::Attachments(commission_id(1));
    record(&host, &attachments, Place::Commission(attachments_place));

    let closed = host.folder(&posts);
    let open = host.folder(&attachments);

    assert!(closed.is_none());
    assert!(matches!(open, Some(Parent::At(_))));
}

#[tokio::test]
async fn a_recorded_commission_s_thing_is_its_place_s_id_not_its_path_s_last_segment() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let recorded_commission = commission_id(1);
    let another_commission = commission_id(2);
    let at = path(&["commission", &another_commission.to_string()]);
    let place = commission::den::CommissionPlace::Commission(recorded_commission);
    record(&host, &at, Place::Commission(place));
    let expected_thing = Some(DenThing::Commission(recorded_commission));

    let thing = host.recorded(&at).and_then(|facts| facts.thing);

    assert_eq!(thing, expected_thing);
}

#[tokio::test]
async fn a_recorded_account_s_thing_is_its_place_s_did_not_its_path_s_last_segment() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let recorded_account = AccountId::from(Did::from("did:plc:recorded".to_owned()));
    let at = path(&["account", "did:plc:another"]);
    let place = account::den::AccountPlace::Account(recorded_account.clone());
    record(&host, &at, Place::Account(place));
    let expected_thing = Some(DenThing::Account(recorded_account));

    let thing = host.recorded(&at).and_then(|facts| facts.thing);

    assert_eq!(thing, expected_thing);
}

/// A live, Private node at `place`.
fn node_at(place: Place) -> Node {
    Node {
        place,
        name: NodeName::try_from("a node").expect("a valid test name"),
        level: Level::Private,
        lifts: Vec::new(),
        state: NodeState::Live,
    }
}

/// A live node for the commission `commission`.
fn commission_node(commission: CommissionId) -> Node {
    let place = commission::den::CommissionPlace::Commission(commission);
    node_at(Place::Commission(place))
}

#[tokio::test]
async fn a_node_that_cannot_sit_in_its_folder_is_dropped_unrecorded() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let user_directory = path(&["user"]);
    let folder = Parent::At(Place::TopLevel(TopLevel::User));

    let found = host.found(&user_directory, &folder, commission_node(commission_id(7)));

    assert_eq!(found, None);
    assert_eq!(host.record().len(), 0);
}

#[tokio::test]
async fn a_node_that_fits_its_folder_is_recorded_under_its_key() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let commission_directory = path(&["commission"]);
    let folder = Parent::At(Place::TopLevel(TopLevel::Commission));
    let commission = commission_id(7);

    let found = host.found(&commission_directory, &folder, commission_node(commission));

    let key = segment(&commission.to_string());
    assert!(found.is_some_and(|found| found.metadata.key == key));
    assert!(host.record().contains_key(&commission_directory.join(key)));
}

#[test]
fn an_answer_for_another_key_is_dropped() {
    let commission = commission_id(7);

    let asked_for_it = answered_for(
        Some(commission_node(commission)),
        &segment(&commission.to_string()),
    );
    let asked_for_another = answered_for(Some(commission_node(commission)), &segment("c1"));

    assert!(asked_for_it.is_some());
    assert_eq!(asked_for_another, None);
}

#[tokio::test]
async fn the_posts_mount_in_a_root_is_answered_only_for_the_session_s_viewer() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let bob = DenHost::viewer_of(&UserId::from(Did::from("did:plc:bob".to_owned())));
    let root = path(&["user", "did:plc:alice"]);
    record(
        &host,
        &root,
        Place::User(user::den::UserPlace::User(alice.clone())),
    );

    let own = host
        .mounts(host.viewer(), &root)
        .await
        .expect("no store is read");
    let another = host.mounts(&bob, &root).await.expect("no store is read");

    let posts = Mount {
        segment: segment("posts"),
        target: path(&["post", "did:plc:alice"]),
    };
    assert_eq!(own, [posts]);
    assert_eq!(another, Vec::new());
}

#[tokio::test]
async fn an_unrecorded_root_mounts_nothing() {
    let ports = ports();
    let alice = alice();
    let host = DenHost::new(&ports, &alice, false);
    let viewer = host.viewer().clone();
    let root = path(&["user", "did:plc:alice"]);

    let mounts = host.mounts(&viewer, &root).await.expect("no store is read");

    assert_eq!(mounts, Vec::new());
}

/// alice's host over the fixture world, with `include_deleted` as given, and the real paths
/// of her commission mounts' targets after locating each.
async fn alice_s_commission_mounts(
    ports: &Ports,
    alice: &UserId,
    include_deleted: bool,
) -> Vec<(String, Admission, Option<Option<SoftDelete>>)> {
    let host = DenHost::new(ports, alice, include_deleted);
    let viewer = host.viewer().clone();
    let folder = path(&["user", "did:plc:alice", "commissions"]);
    let commissions_place = commission::den::CommissionPlace::UserCommissions(alice.clone());
    record(&host, &folder, Place::Commission(commissions_place));
    let mounts = host.mounts(&viewer, &folder).await.expect("no store fails");
    host.child(&viewer, &RealPath::default(), &segment("commission"))
        .await
        .expect("no store fails");
    let mut answers = Vec::new();
    for mount in mounts {
        let [top_level, key] = mount.target.segments() else {
            panic!("a mount targets a node under a top-level directory");
        };
        let found = host
            .child(&viewer, &path(&[top_level.as_ref()]), key)
            .await
            .expect("no store fails")
            .expect("the mounted commission exists");
        let admission = host.admit(
            &viewer,
            &mount.target,
            &found.metadata,
            open_by_standing(),
            Reach::Listing,
        );
        let word = host.recorded(&mount.target).map(|facts| facts.soft_delete);
        answers.push((found.metadata.name.to_string(), admission, word));
    }
    answers
}

#[tokio::test]
async fn an_archived_commission_lists_only_when_deleted_nodes_are_asked_for() {
    let World { ports, .. } = crate::test_world::world().await;
    let alice = adapter_mem::fixture::alice();

    let hidden = alice_s_commission_mounts(&ports, &alice, false).await;
    let shown = alice_s_commission_mounts(&ports, &alice, true).await;

    let expected_hidden = vec![
        (
            "Old sketch".to_owned(),
            Admission::Refuse,
            Some(Some(SoftDelete::Archived)),
        ),
        ("Untitled".to_owned(), Admission::Admit, Some(None)),
    ];
    let expected_shown = vec![
        (
            "Old sketch".to_owned(),
            Admission::Admit,
            Some(Some(SoftDelete::Archived)),
        ),
        ("Untitled".to_owned(), Admission::Admit, Some(None)),
    ];
    assert_eq!(hidden, expected_hidden);
    assert_eq!(shown, expected_shown);
}

#[tokio::test]
async fn another_user_s_commissions_folder_mounts_nothing_for_the_viewer() {
    let World { ports, .. } = crate::test_world::world().await;
    let alice = adapter_mem::fixture::alice();
    let host = DenHost::new(&ports, &alice, true);
    let viewer = host.viewer().clone();
    let bobs_folder = path(&["user", "did:plc:bob", "commissions"]);
    let bobs_place = commission::den::CommissionPlace::UserCommissions(adapter_mem::fixture::bob());
    record(&host, &bobs_folder, Place::Commission(bobs_place));

    let mounts = host
        .mounts(&viewer, &bobs_folder)
        .await
        .expect("no store fails");

    assert_eq!(mounts, Vec::new());
}

#[tokio::test]
async fn a_recorded_archived_node_lists_only_when_deleted_nodes_are_asked_for() {
    let ports = ports();
    let alice = alice();
    let archived = path(&["commission", "c1"]);
    let mut answers = Vec::new();
    for include_deleted in [false, true] {
        let host = DenHost::new(&ports, &alice, include_deleted);
        let viewer = host.viewer().clone();
        let recorded = Recorded {
            place: Place::TopLevel(TopLevel::Commission),
            state: NodeState::Archived,
        };
        host.record().insert(archived.clone(), recorded);
        let metadata = Metadata {
            key: segment("c1"),
            parent: Some(path(&["commission"])),
            typ: type_key(DenType::Commission),
            level: atta::Level::Private,
            name: display_name("Old sketch"),
            symlink_target: None,
        };

        let listing = host.admit(
            &viewer,
            &archived,
            &metadata,
            open_by_standing(),
            Reach::Listing,
        );
        let on_a_path = host.admit(
            &viewer,
            &archived,
            &metadata,
            open_by_standing(),
            Reach::Path,
        );

        answers.push((include_deleted, listing, on_a_path));
    }

    let expected = vec![
        (false, Admission::Refuse, Admission::Admit),
        (true, Admission::Admit, Admission::Admit),
    ];
    assert_eq!(answers, expected);
}

/// `text` as the engine's display name, as the host hands it over.
#[allow(
    clippy::disallowed_methods,
    reason = "a test builds the metadata the host's one construction site would"
)]
fn display_name(text: &str) -> DisplayName {
    DisplayName::host_checked(text.to_owned())
}
