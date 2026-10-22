//! What the resolver logs: each resolve and list names its outcome class and depth at debug,
//! never a path, a segment or the viewer. Alone in its binary: tracing caches a callsite's
//! interest, so a sibling test hitting these callsites first could blind the capture.

use std::sync::{Arc, Mutex};

use atta::{
    Access, Admission, Caps, DisplayName, Found, Host, HostError, Kind, Level, Lift, Metadata,
    Mount, Reach, RealPath, Relationship, Resolver, Scope, Segment, Type, Viewer,
};
use tracing_subscriber::fmt::MakeWriter;

/// What a test subscriber wrote; clones share one buffer.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    fn text(&self) -> String {
        let bytes = self.0.lock().expect("unpoisoned").clone();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

impl std::io::Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("unpoisoned").extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for Captured {
    type Writer = Self;

    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

/// Standing at a node alone: it opens, nothing below it does by it.
const STANDING_HERE: Lift = Lift {
    level: Level::Public,
    scope: Scope::Node,
    relationship: Relationship::Standing,
};

/// A grant that shows a node as a card.
const CARD_GRANT: Lift = Lift {
    level: Level::Listed,
    scope: Scope::Node,
    relationship: Relationship::Grant,
};

/// A host over a fixed tree: alice's root (open by her standing), a public `works` folder in
/// it holding the public `piece` and the card `secret`; every call fails when `failing`.
struct TreeHost {
    nodes: Vec<(RealPath, Found)>,
    failing: bool,
}

fn segment(text: &str) -> Segment {
    Segment::try_from(text).expect("a valid test segment")
}

fn path(pieces: &[&str]) -> RealPath {
    pieces.iter().map(|piece| segment(piece)).collect()
}

/// A Den path below the viewer's root.
fn den(pieces: &[&str]) -> Vec<Segment> {
    pieces.iter().map(|piece| segment(piece)).collect()
}

/// The node at `pieces`, of its own `level`, with the viewer's `lifts` there.
fn node(pieces: &[&str], level: Level, lifts: &[Lift]) -> (RealPath, Found) {
    let real_path = path(pieces);
    let (key, parent) = real_path.segments().split_last().expect("below the root");
    let metadata = Metadata {
        key: key.clone(),
        parent: Some(parent.iter().cloned().collect()),
        typ: Type::from("folder".to_owned()),
        level,
        name: DisplayName::host_checked("a node".to_owned()),
        symlink_target: None,
    };
    let found = Found {
        metadata,
        lifts: lifts.to_vec(),
    };
    (real_path, found)
}

impl TreeHost {
    fn new(failing: bool) -> Self {
        let nodes = vec![
            node(&["user"], Level::Public, &[]),
            node(&["user", "alice"], Level::Private, &[STANDING_HERE]),
            node(&["user", "alice", "works"], Level::Public, &[]),
            node(&["user", "alice", "works", "piece"], Level::Public, &[]),
            node(
                &["user", "alice", "works", "secret"],
                Level::Private,
                &[CARD_GRANT],
            ),
        ];
        Self { nodes, failing }
    }

    fn check(&self) -> Result<(), HostError> {
        if self.failing {
            let failure: Box<dyn std::error::Error + Send + Sync> = "the store is down".into();
            return Err(HostError::from(failure));
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl Host for TreeHost {
    async fn child(
        &self,
        _viewer: &Viewer,
        parent: &RealPath,
        key: &Segment,
    ) -> Result<Option<Found>, HostError> {
        self.check()?;
        let wanted = parent.join(key.clone());
        let child = self
            .nodes
            .iter()
            .find(|(real_path, _)| *real_path == wanted)
            .map(|(_, found)| found.clone());
        Ok(child)
    }

    async fn children(&self, _viewer: &Viewer, parent: &RealPath) -> Result<Vec<Found>, HostError> {
        self.check()?;
        let in_parent = |real_path: &RealPath| {
            real_path
                .segments()
                .split_last()
                .is_some_and(|(_, above)| above == parent.segments())
        };
        let children = self
            .nodes
            .iter()
            .filter(|(real_path, _)| in_parent(real_path))
            .map(|(_, found)| found.clone())
            .collect();
        Ok(children)
    }

    async fn mounts(&self, _viewer: &Viewer, _folder: &RealPath) -> Result<Vec<Mount>, HostError> {
        self.check()?;
        Ok(Vec::new())
    }

    fn kind(&self, _typ: &Type) -> Kind {
        Kind::Directory
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

fn resolver(host: &TreeHost) -> Resolver<'_> {
    Resolver {
        host,
        viewer: Viewer::from("alice".to_owned()),
        caps: Caps::default(),
    }
}

/// The line logging `event` with `outcome`, among `text`'s.
fn outcome_line<'t>(text: &'t str, event: &str, outcome: &str) -> Option<&'t str> {
    let field = format!("outcome=\"{outcome}\"");
    text.lines()
        .find(|line| line.contains(event) && line.contains(field.as_str()))
}

#[tokio::test]
async fn each_read_logs_its_outcome_class_and_depth_and_never_its_path() {
    let host = TreeHost::new(false);
    let failing = TreeHost::new(true);
    let root = path(&["user", "alice"]);
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(captured.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let opened = resolver(&host)
        .resolve_path(&root, &den(&["works", "piece"]))
        .await;
    let carded = resolver(&host)
        .resolve_path(&root, &den(&["works", "secret"]))
        .await;
    let missing = resolver(&host)
        .resolve_path(&root, &den(&["nowhere"]))
        .await;
    let broken = resolver(&failing)
        .resolve_path(&root, &den(&["works"]))
        .await;
    let folder = resolver(&host)
        .resolve_path(&root, &den(&["works"]))
        .await
        .expect("the folder opens");
    let listed = resolver(&host).list(&folder, None).await;

    assert!(opened.is_ok() && carded.is_ok() && missing.is_err() && broken.is_err());
    assert!(listed.is_ok_and(|listing| listing.is_some()));
    let text = captured.text();
    let open = outcome_line(&text, "resolved", "open").unwrap_or_else(|| panic!("{text}"));
    assert!(open.contains("depth=2") && open.contains("DEBUG"), "{open}");
    let card = outcome_line(&text, "resolved", "card").unwrap_or_else(|| panic!("{text}"));
    assert!(card.contains("depth=2"), "{card}");
    let not_found =
        outcome_line(&text, "resolved", "not_found").unwrap_or_else(|| panic!("{text}"));
    assert!(not_found.contains("depth=1"), "{not_found}");
    let host_error =
        outcome_line(&text, "resolved", "host_error").unwrap_or_else(|| panic!("{text}"));
    assert!(host_error.contains("depth=1"), "{host_error}");
    let list = outcome_line(&text, "listed", "open").unwrap_or_else(|| panic!("{text}"));
    assert!(
        list.contains("depth=1") && list.contains("entries=2"),
        "{list}"
    );
    for planted in [
        "alice",
        "works",
        "piece",
        "secret",
        "nowhere",
        "store is down",
    ] {
        assert!(!text.contains(planted), "the log carries {planted}: {text}");
    }
}
