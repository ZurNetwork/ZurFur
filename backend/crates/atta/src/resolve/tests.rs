use super::*;
use crate::fake::{Call, CallShape, FakeHost, GRANT_CARD, GRANT_NODE, STANDING, path, segment};

/// The view of a node the viewer opens, showing `own_level`.
fn open(own_level: Level) -> View {
    View::Open { own_level }
}

/// The access a viewer holds at `level`, given by `via`.
fn expected_access(level: Level, via: Via) -> Access {
    Access { level, via }
}

/// alice's root, two folders and a file in it, and her mounts of six commissions, each reached
/// through a different relationship.
fn world() -> FakeHost {
    FakeHost::default()
        .top_level()
        .file_type("file")
        .node("/user/alice", "user", Level::Private, &[STANDING])
        .node(
            "/user/alice/commissions",
            "user.commissions",
            Level::Private,
            &[],
        )
        .node("/user/alice/readme", "file", Level::Private, &[])
        .mount("/user/alice/commissions", "c1", "/commission/c1")
        .mount(
            "/user/alice/commissions",
            "stranger",
            "/commission/stranger",
        )
        .mount("/user/alice/commissions", "public", "/commission/public")
        .mount("/user/alice/commissions", "granted", "/commission/granted")
        .mount("/user/alice/commissions", "card", "/commission/card")
        .mount("/user/alice/commissions", "both", "/commission/both")
        .node("/commission/c1", "commission", Level::Private, &[STANDING])
        .node(
            "/commission/c1/attachments",
            "commission.attachments",
            Level::Private,
            &[],
        )
        .node("/commission/c1/attachments/f1", "file", Level::Private, &[])
        .node("/commission/stranger", "commission", Level::Private, &[])
        .node("/commission/public", "commission", Level::Public, &[])
        .node(
            "/commission/public/attachments",
            "commission.attachments",
            Level::Private,
            &[],
        )
        .node(
            "/commission/granted",
            "commission",
            Level::Private,
            &[GRANT_NODE],
        )
        .node(
            "/commission/granted/attachments",
            "commission.attachments",
            Level::Public,
            &[],
        )
        .node(
            "/commission/card",
            "commission",
            Level::Private,
            &[GRANT_CARD],
        )
        .node(
            "/commission/card/attachments",
            "commission.attachments",
            Level::Public,
            &[],
        )
        .node("/commission/both", "commission", Level::Public, &[STANDING])
        .node(
            "/commission/secret",
            "commission",
            Level::Private,
            &[STANDING],
        )
}

fn root() -> RealPath {
    path("/user/alice")
}

fn den(pieces: &[&str]) -> Vec<Segment> {
    pieces.iter().map(|piece| segment(piece)).collect()
}

fn resolver(host: &FakeHost) -> Resolver<'_> {
    Resolver {
        host,
        viewer: Viewer::from("alice".to_owned()),
        caps: Caps::default(),
    }
}

async fn resolve(host: &FakeHost, pieces: &[&str]) -> Result<Resolved, Error> {
    resolver(host).resolve(&root(), &den(pieces)).await
}

async fn assert_not_found(host: &FakeHost, pieces: &[&str]) {
    let result = resolve(host, pieces).await;
    assert!(
        matches!(result, Err(Error::NotFound)),
        "{pieces:?} gave {result:?}"
    );
}

#[tokio::test]
async fn the_root_opens_through_the_viewers_own_standing() {
    let host = world();
    let resolved = resolve(&host, &[]).await.expect("the root opens");
    let expected_view = open(Level::Private);
    assert_eq!(resolved.node.view, expected_view);
    assert_eq!(resolved.node.real_path, root());
    assert!(resolved.node.den_path.is_empty());
    assert!(resolved.crumbs.is_empty());
    let standing = expected_access(Level::Public, Via::Standing);
    assert_eq!(host.admits(), [(root(), standing, Reach::Path)]);
}

#[tokio::test]
async fn standing_covers_the_real_subtree() {
    let host = world();
    let resolved = resolve(&host, &["commissions"])
        .await
        .expect("a folder in her own root opens");
    assert_eq!(resolved.node.view, open(Level::Private));
    let (_, access, _) = host.admits().pop().expect("the folder was admitted");
    assert_eq!(access, expected_access(Level::Public, Via::Standing));
}

#[tokio::test]
async fn standing_never_crosses_a_mount() {
    let host = world();
    assert_not_found(&host, &["commissions", "stranger"]).await;
}

#[tokio::test]
async fn a_participants_standing_opens_the_mounted_subtree() {
    let host = world();
    let resolved = resolve(&host, &["commissions", "c1", "attachments", "f1"])
        .await
        .expect("her file opens");
    assert_eq!(
        resolved.node.real_path,
        path("/commission/c1/attachments/f1")
    );
    assert_eq!(
        resolved.node.den_path,
        den(&["commissions", "c1", "attachments", "f1"])
    );
    assert_eq!(resolved.node.kind, Kind::File);
    assert!(!resolved.node.mount);
    let crumb_paths: Vec<_> = resolved
        .crumbs
        .iter()
        .map(|crumb| crumb.real_path.to_string())
        .collect();
    let expected_crumb_paths = [
        "/user/alice",
        "/user/alice/commissions",
        "/commission/c1",
        "/commission/c1/attachments",
    ];
    assert_eq!(crumb_paths, expected_crumb_paths);
    let mount_flags: Vec<_> = resolved.crumbs.iter().map(|crumb| crumb.mount).collect();
    assert_eq!(mount_flags, [false, false, true, false]);
    assert_eq!(resolved.crumbs[2].den_path, den(&["commissions", "c1"]));
}

#[tokio::test]
async fn a_public_node_opens_through_its_own_level() {
    let host = world();
    let resolved = resolve(&host, &["commissions", "public"])
        .await
        .expect("a public node opens");
    assert_eq!(resolved.node.view, open(Level::Public));
    let (_, access, _) = host.admits().pop().expect("the node was admitted");
    assert_eq!(access, expected_access(Level::Public, Via::OwnLevel));
}

#[tokio::test]
async fn a_public_nodes_private_child_stays_absent() {
    let host = world();
    assert_not_found(&host, &["commissions", "public", "attachments"]).await;
}

#[tokio::test]
async fn a_node_scoped_grant_opens_the_node_alone() {
    let host = world();
    let resolved = resolve(&host, &["commissions", "granted"])
        .await
        .expect("the granted node opens");
    assert_eq!(resolved.node.view, open(Level::Private));
    let (_, access, _) = host.admits().pop().expect("the node was admitted");
    assert_eq!(access, expected_access(Level::Public, Via::Grant));
    // Its public child opens by its own level, not by the grant.
    let child = resolve(&host, &["commissions", "granted", "attachments"])
        .await
        .expect("a public child opens");
    let (_, child_access, _) = host.admits().pop().expect("the child was admitted");
    assert_eq!(child.node.view, open(Level::Public));
    assert_eq!(child_access.via, Via::OwnLevel);
}

#[tokio::test]
async fn a_listed_node_is_a_card_and_nothing_inside_it_opens() {
    let host = world();
    let resolved = resolve(&host, &["commissions", "card"])
        .await
        .expect("a card answers");
    assert_eq!(resolved.node.view, View::Card);
    let (_, access, reach) = host.admits().pop().expect("the card was admitted");
    assert_eq!(access, expected_access(Level::Listed, Via::Grant));
    assert_eq!(reach, Reach::Path);
    assert_not_found(&host, &["commissions", "card", "attachments"]).await;
}

#[tokio::test]
async fn a_card_on_the_way_down_is_never_offered_to_admit() {
    let host = world();
    assert_not_found(&host, &["commissions", "card", "attachments"]).await;
    let admitted: Vec<_> = host
        .admits()
        .into_iter()
        .map(|(path, _, _)| path.to_string())
        .collect();
    assert_eq!(admitted, ["/user/alice", "/user/alice/commissions"]);
}

#[tokio::test]
async fn a_relationship_wins_a_tie_with_the_own_level() {
    let host = world();
    resolve(&host, &["commissions", "both"])
        .await
        .expect("the node opens");
    let (_, access, _) = host.admits().pop().expect("the node was admitted");
    assert_eq!(access, expected_access(Level::Public, Via::Standing));
}

#[tokio::test]
async fn every_miss_is_the_one_not_found() {
    let misses: [(&str, FakeHost, &[&str]); 14] = [
        ("absent", world(), &["nothing"]),
        (
            "below a top-level node that is not a directory",
            world().file_type("top"),
            &[],
        ),
        (
            "absent inside a mount",
            world(),
            &["commissions", "c1", "nothing"],
        ),
        ("private", world(), &["commissions", "stranger"]),
        (
            "inside a node the viewer can't open",
            world(),
            &["commissions", "stranger", "attachments"],
        ),
        ("past a file", world(), &["readme", "more"]),
        (
            "past a card",
            world(),
            &["commissions", "card", "attachments"],
        ),
        (
            "refused on the path",
            world().refuse("/commission/c1", Reach::Path),
            &["commissions", "c1"],
        ),
        (
            "a card the host refuses",
            world().refuse("/commission/card", Reach::Path),
            &["commissions", "card"],
        ),
        (
            "refused above the node",
            world().refuse("/commission/c1", Reach::Path),
            &["commissions", "c1", "attachments"],
        ),
        (
            "a mount and a real child at one segment",
            world().node("/user/alice/commissions/c1", "x", Level::Public, &[]),
            &["commissions", "c1"],
        ),
        (
            "two mounts at one segment",
            world().mount("/user/alice/commissions", "c1", "/commission/public"),
            &["commissions", "c1"],
        ),
        (
            "a node the host places elsewhere",
            world().misplaced("/user/alice/commissions", "/user/bob"),
            &["commissions"],
        ),
        (
            "a symlink",
            world().linked("/user/alice/commissions", "/commission/c1"),
            &["commissions"],
        ),
    ];
    for (cause, host, pieces) in misses {
        let result = resolve(&host, pieces).await;
        assert!(
            matches!(result, Err(Error::NotFound)),
            "{cause}: {result:?}"
        );
    }
}

#[tokio::test]
async fn a_mount_whose_target_is_not_directly_under_a_top_level_directory_fails_closed() {
    let too_deep = world().mount(
        "/user/alice/commissions",
        "deep",
        "/commission/c1/attachments",
    );
    let too_shallow = world().mount("/user/alice/commissions", "shallow", "/commission");
    assert_not_found(&too_deep, &["commissions", "deep"]).await;
    assert_not_found(&too_shallow, &["commissions", "shallow"]).await;
}

#[tokio::test]
async fn mounts_are_matched_in_the_viewers_own_set_before_any_target_is_looked_up() {
    let host = world();
    assert_not_found(&host, &["commissions", "secret"]).await;
    let looked_up_commissions = host
        .calls()
        .into_iter()
        .any(|call| matches!(call, Call::Child { parent, .. } if parent == path("/commission")));
    assert!(
        !looked_up_commissions,
        "the requested target was looked up before the mount set was matched"
    );
}

#[tokio::test]
async fn an_absent_node_and_a_hidden_one_make_the_same_host_calls() {
    let absent_host = world();
    let hidden_host = world();
    assert_not_found(&absent_host, &["commissions", "public", "nothing"]).await;
    assert_not_found(&hidden_host, &["commissions", "public", "attachments"]).await;
    let shapes = |host: &FakeHost| host.calls().iter().map(Call::shape).collect::<Vec<_>>();
    assert_eq!(shapes(&absent_host), shapes(&hidden_host));
}

#[tokio::test]
async fn a_refusal_on_the_path_adds_only_the_admit_call_to_an_absent_nodes_calls() {
    let absent_host = world();
    let refused_host = world()
        .node("/commission/public/files", "folder", Level::Public, &[])
        .refuse("/commission/public/files", Reach::Path);
    assert_not_found(&absent_host, &["commissions", "public", "nothing"]).await;
    assert_not_found(&refused_host, &["commissions", "public", "files"]).await;
    let shapes = |host: &FakeHost| host.calls().iter().map(Call::shape).collect::<Vec<_>>();
    let mut absent_then_admit = shapes(&absent_host);
    absent_then_admit.push(CallShape::Admit(3, Reach::Path));
    assert_eq!(shapes(&refused_host), absent_then_admit);
}

#[tokio::test]
async fn a_path_over_the_cap_is_not_found_before_the_host_is_asked() {
    let host = world();
    let pieces = ["commissions"; 17];
    assert_not_found(&host, &pieces).await;
    assert!(host.calls().is_empty());
}

#[tokio::test]
async fn a_root_not_directly_under_a_top_level_directory_is_never_resolved() {
    let host = world();
    for root in ["/", "/user", "/user/alice/commissions"] {
        let result = resolver(&host).resolve(&path(root), &[]).await;
        assert!(matches!(result, Err(Error::NotFound)), "{root}: {result:?}");
    }
}

#[tokio::test]
async fn a_host_failure_is_not_a_miss() {
    let host = world().failing();
    let result = resolve(&host, &[]).await;
    assert!(matches!(result, Err(Error::Host(_))), "{result:?}");
}

#[test]
fn the_highest_level_wins() {
    let card = Lift {
        level: Level::Listed,
        scope: Scope::Node,
        relationship: Relationship::Grant,
    };
    assert_eq!(
        access(Level::Private, &[], &[]),
        expected_access(Level::Private, Via::OwnLevel)
    );
    assert_eq!(
        access(Level::Public, &[card], &[]),
        expected_access(Level::Public, Via::OwnLevel)
    );
    assert_eq!(
        access(Level::Private, &[card], &[]),
        expected_access(Level::Listed, Via::Grant)
    );
    assert_eq!(
        access(Level::Listed, &[], &[STANDING]),
        expected_access(Level::Public, Via::Standing)
    );
}

#[test]
fn standing_wins_a_tie_with_a_grant() {
    assert_eq!(
        access(Level::Private, &[GRANT_NODE], &[STANDING]),
        expected_access(Level::Public, Via::Standing)
    );
}
