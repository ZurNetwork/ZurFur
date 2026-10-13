use std::{collections::HashMap, str::FromStr, sync::Mutex};

use atta::{
    Access, Admission, Found, HostError, Lift, Metadata, Mount, NodeName, Reach, Relationship,
    Scope, Type,
};

use super::*;
use crate::den::{DenAccess, DenContent};

fn segment(text: &str) -> Segment {
    Segment::try_from(text).expect("a valid test segment")
}

fn path(text: &str) -> RealPath {
    text.split('/')
        .filter(|piece| !piece.is_empty())
        .map(segment)
        .collect()
}

fn pieces(texts: &[&str]) -> Vec<String> {
    texts.iter().map(|text| (*text).to_owned()).collect()
}

#[test]
fn an_empty_den_path_is_not_found() {
    let parsed = den_path(&[], Caps::default());

    assert!(matches!(parsed, Err(DenError::NotFound)));
}

#[test]
fn seventeen_pieces_are_not_found() {
    let seventeen = vec!["a".to_owned(); 17];

    let parsed = den_path(&seventeen, Caps::default());

    assert!(matches!(parsed, Err(DenError::NotFound)));
}

#[test]
fn one_piece_is_its_segment() {
    let parsed = den_path(&pieces(&["accounts"]), Caps::default());

    let expected = vec![segment("accounts")];
    assert!(matches!(parsed, Ok(segments) if segments == expected));
}

#[test]
fn sixteen_pieces_are_their_segments_in_order() {
    let texts: Vec<String> = (0..16).map(|index| format!("p{index}")).collect();

    let parsed = den_path(&texts, Caps::default());

    let expected: Vec<Segment> = texts.iter().map(|text| segment(text)).collect();
    assert!(matches!(parsed, Ok(segments) if segments == expected));
}

#[test]
fn each_malformed_piece_is_not_found() {
    let too_long = "a".repeat(2049);
    for piece in ["", ".", "..", "a/b", "a b", too_long.as_str()] {
        let parsed = den_path(&pieces(&["accounts", piece]), Caps::default());

        assert!(matches!(parsed, Err(DenError::NotFound)), "{piece:?}");
    }
}

/// Standing over a subtree, as a user has at their own root.
const STANDING: Lift = Lift {
    level: Level::Public,
    scope: Scope::Subtree,
    relationship: Relationship::Standing,
};

/// A host over an in-memory tree that admits everything and records which folders it listed.
#[derive(Default)]
struct FakeHost {
    nodes: Vec<(RealPath, Found)>,
    mounts: HashMap<RealPath, Vec<Mount>>,
    words: HashMap<RealPath, SoftDelete>,
    listed: Mutex<Vec<RealPath>>,
}

impl FakeHost {
    /// Adds the node at `at`, of `den_type`, with its own `level` and the viewer's `lifts`.
    fn node(mut self, at: &str, den_type: DenType, level: Level, lifts: &[Lift]) -> Self {
        let real_path = path(at);
        let (key, parent) = real_path
            .segments()
            .split_last()
            .expect("a node below the root");
        let metadata = Metadata {
            key: key.clone(),
            parent: Some(parent.iter().cloned().collect()),
            typ: Type::from(den_type.to_string()),
            level,
            name: NodeName::try_from(format!("name of {key}")).expect("a valid test name"),
            symlink_target: None,
        };
        let found = Found {
            metadata,
            lifts: lifts.to_vec(),
        };
        self.nodes.push((real_path, found));
        self
    }

    /// Adds the viewer's mount `segment` in `folder`, grafting in `target`.
    fn mount(mut self, folder: &str, segment_text: &str, target: &str) -> Self {
        let mount = Mount {
            segment: segment(segment_text),
            target: path(target),
        };
        self.mounts.entry(path(folder)).or_default().push(mount);
        self
    }

    /// Marks the node at `at` soft-deleted with `word`.
    fn soft_deleted(mut self, at: &str, word: SoftDelete) -> Self {
        self.words.insert(path(at), word);
        self
    }

    /// Whether `children` was asked for the folder at `at`.
    fn listed(&self, at: &str) -> bool {
        let listed = self.listed.lock().expect("an unpoisoned lock");
        listed.contains(&path(at))
    }
}

#[async_trait::async_trait]
impl Host for FakeHost {
    async fn child(
        &self,
        _viewer: &Viewer,
        parent: &RealPath,
        key: &Segment,
    ) -> Result<Option<Found>, HostError> {
        let wanted = parent.join(key.clone());
        let found = self
            .nodes
            .iter()
            .find(|(at, _)| *at == wanted)
            .map(|(_, found)| found.clone());
        Ok(found)
    }

    async fn children(&self, _viewer: &Viewer, parent: &RealPath) -> Result<Vec<Found>, HostError> {
        self.listed
            .lock()
            .expect("an unpoisoned lock")
            .push(parent.clone());
        let children = self
            .nodes
            .iter()
            .filter(|(_, found)| found.metadata.parent.as_ref() == Some(parent))
            .map(|(_, found)| found.clone())
            .collect();
        Ok(children)
    }

    async fn mounts(&self, _viewer: &Viewer, folder: &RealPath) -> Result<Vec<Mount>, HostError> {
        let mounts = self.mounts.get(folder).cloned().unwrap_or_default();
        Ok(mounts)
    }

    fn kind(&self, typ: &Type) -> Kind {
        let den_type = DenType::from_str(typ.as_ref()).expect("a known test type");
        match den_type.kind() {
            NodeKind::Directory => Kind::Directory,
            NodeKind::File | NodeKind::Symlink => Kind::File,
        }
    }

    fn admit(
        &self,
        _viewer: &Viewer,
        _path: &RealPath,
        _node: &Metadata,
        _access: Access,
        _reach: Reach,
    ) -> Admission {
        Admission::Admit
    }
}

impl SoftDeleteLookup for FakeHost {
    fn soft_delete(&self, path: &RealPath) -> Option<Option<SoftDelete>> {
        Some(self.words.get(path).copied())
    }
}

/// alice's root, with one node of each shape the read answers for.
fn tree() -> FakeHost {
    FakeHost::default()
        .node("/user", DenType::TopLevel, Level::Public, &[])
        .node("/commission", DenType::TopLevel, Level::Public, &[])
        .node("/user/alice", DenType::User, Level::Private, &[STANDING])
        .node("/user/alice/flagged", DenType::Post, Level::Private, &[])
        .node(
            "/user/alice/flagged/inside",
            DenType::File,
            Level::Private,
            &[],
        )
        .node("/user/alice/notes", DenType::File, Level::Private, &[])
        .node(
            "/user/alice/private",
            DenType::UserAccounts,
            Level::Private,
            &[],
        )
        .node(
            "/user/alice/listed",
            DenType::UserAccounts,
            Level::Listed,
            &[],
        )
        .node(
            "/user/alice/public",
            DenType::UserAccounts,
            Level::Public,
            &[],
        )
        .node("/user/alice/old", DenType::Commission, Level::Private, &[])
        .soft_deleted("/user/alice/old", SoftDelete::Archived)
        .node("/commission/c1", DenType::Commission, Level::Listed, &[])
        .node("/commission/c2", DenType::Commission, Level::Public, &[])
        .soft_deleted("/commission/c2", SoftDelete::Archived)
        .mount("/user/alice", "card", "/commission/c1")
        .mount("/user/alice", "gone", "/commission/c2")
}

/// Reads the node at `den_path` below alice's root over `host`.
async fn read(host: &FakeHost, den_path: &[&str]) -> (DenNode, Option<DenContent>) {
    let viewer = Viewer::from("did:plc:alice".to_owned());
    let reader = Reader::new(host, viewer, host, Caps::default());
    let segments: Vec<Segment> = den_path.iter().map(|text| segment(text)).collect();
    let resolved = reader
        .walk(&path("/user/alice"), &segments)
        .await
        .expect("the test path resolves");
    let node = reader.node(resolved.node()).expect("a known type");
    let content = reader
        .content(&resolved, None)
        .await
        .expect("the content reads");
    (node, content)
}

#[tokio::test]
async fn a_flagged_open_directory_lists_nothing_and_never_asks_for_its_children() {
    let host = tree();

    let (node, content) = read(&host, &["flagged"]).await;

    let empty_listing = DenContent::Listing(DenListing::default());
    assert_eq!(content, Some(empty_listing));
    assert!(!host.listed("/user/alice/flagged"));
    assert!(matches!(
        node.access,
        DenAccess::Open {
            content_not_shown: true,
            ..
        }
    ));
}

#[tokio::test]
async fn an_unflagged_open_directory_lists_its_children() {
    let host = tree();

    let (_, content) = read(&host, &[]).await;

    assert!(host.listed("/user/alice"));
    assert!(matches!(
        content,
        Some(DenContent::Listing(listing)) if !listing.entries.is_empty()
    ));
}

#[tokio::test]
async fn a_card_has_no_content_and_no_segments() {
    let host = tree();

    let (node, content) = read(&host, &["card"]).await;

    assert_eq!(node.access, DenAccess::Card);
    assert_eq!(node.segments, Vec::new());
    assert!(node.mount);
    assert_eq!(content, None);
}

#[tokio::test]
async fn a_card_has_no_listing() {
    let host = tree();
    let viewer = Viewer::from("did:plc:alice".to_owned());
    let reader = Reader::new(&host, viewer, &host, Caps::default());
    let resolved = reader
        .walk(&path("/user/alice"), &[segment("card")])
        .await
        .expect("the card resolves");

    let listing = reader.listing(&resolved, None).await;

    assert!(matches!(listing, Err(DenError::NotFound)));
}

#[tokio::test]
async fn an_open_file_answers_a_file() {
    let host = tree();

    let (node, content) = read(&host, &["notes"]).await;

    assert_eq!(content, Some(DenContent::File));
    assert!(matches!(
        node.access,
        DenAccess::Open {
            kind: NodeKind::File,
            ..
        }
    ));
}

#[tokio::test]
async fn each_own_level_shows_its_word() {
    let host = tree();

    for (piece, own) in [
        ("private", NodeLevel::Private),
        ("listed", NodeLevel::Listed),
        ("public", NodeLevel::Public),
    ] {
        let (node, _) = read(&host, &[piece]).await;

        let expected = DenAccess::Open {
            kind: NodeKind::Directory,
            own_level: own,
            content_not_shown: false,
            soft_delete: None,
        };
        assert_eq!(node.access, expected, "{piece}");
        assert_eq!(node.segments.len(), 1, "{piece}");
    }
}

#[tokio::test]
async fn an_open_node_carries_its_soft_deleted_word() {
    let host = tree();

    let (node, _) = read(&host, &["old"]).await;

    assert!(matches!(
        node.access,
        DenAccess::Open {
            soft_delete: Some(SoftDelete::Archived),
            ..
        }
    ));
}

#[tokio::test]
async fn a_mount_shows_its_target_s_soft_deleted_word() {
    let host = tree();

    let (node, _) = read(&host, &["gone"]).await;

    assert!(node.mount);
    assert!(matches!(
        node.access,
        DenAccess::Open {
            soft_delete: Some(SoftDelete::Archived),
            own_level: NodeLevel::Public,
            ..
        }
    ));
}
