use super::*;
use crate::fake::{FakeHost, path, segment};
use crate::{
    Access, ActName, Admission, DisplayName, Found, Metadata, Mount, Placed, Reach, RealPath,
    Segment, Type,
};

/// A viewer whose text no write type may print.
const VIEWER: &str = "did:plc:viewer-planted";

fn writer() -> Writer {
    Writer {
        viewer: Viewer::from(VIEWER.to_owned()),
        caps: WriteCaps::default(),
    }
}

/// A host where `note` nodes are files and every other type is a directory.
fn host() -> FakeHost {
    FakeHost::default().file_type("note")
}

fn placed(at: &str, typ: &str, level: Level) -> Placed {
    Placed {
        path: path(at),
        typ: Type::from(typ.to_owned()),
        level,
    }
}

fn born(key: &str, typ: &str, level: Option<Level>) -> Born {
    Born {
        key: segment(key),
        typ: Type::from(typ.to_owned()),
        name: DisplayName::host_checked(format!("name of {key}")),
        level,
    }
}

/// An act of the node's own type that makes `node`, holding `entries`, in `parent`.
fn create(parent: Placed, node: Born, entries: Vec<Born>) -> Act<()> {
    let typ = node.typ.clone();
    let shape = Shape::Create {
        parent,
        node,
        entries,
    };
    Act {
        name: ActName::from("create".to_owned()),
        typ,
        shape,
        body: (),
    }
}

/// An act of type `typ` with `shape`.
fn act_of(typ: &str, shape: Shape) -> Act<()> {
    Act {
        name: ActName::from("act".to_owned()),
        typ: Type::from(typ.to_owned()),
        shape,
        body: (),
    }
}

/// The refusal `act` meets on `host`, which must not have applied it.
async fn refusal(host: &mut FakeHost, act: Act<()>) -> Refusal {
    let written = writer().act(host, act).await;
    let Err(WriteError::Refused(refusal)) = written else {
        panic!("expected a refusal, got {written:?}");
    };
    assert!(host.applied().is_empty(), "a refused act reached the host");
    refusal
}

/// Each node the applied `act` made, with its settled level, as (key, level).
async fn settled(host: &mut FakeHost, act: Act<()>) -> Vec<(String, Level)> {
    let written = writer().act(host, act).await;
    assert!(
        written.is_ok(),
        "expected the act to apply, got {written:?}"
    );
    let checked = host.applied().last().expect("the act was applied");
    checked
        .born()
        .map(|(born, level)| (born.key.as_ref().to_owned(), level))
        .collect()
}

/// A path `depth` real segments deep: `/user/alice/d3/…`.
fn path_at_depth(depth: usize) -> String {
    let pieces = ["user", "alice"]
        .into_iter()
        .map(str::to_owned)
        .chain((3..=depth).map(|step| format!("d{step}")));
    pieces
        .take(depth)
        .fold(String::new(), |text, piece| format!("{text}/{piece}"))
}

#[tokio::test]
async fn a_child_under_a_file_is_refused() {
    let mut host = host();
    let parent = placed("/user/alice/readme", "note", Level::Public);
    let act = create(parent, born("inside", "folder", None), Vec::new());

    let refused = refusal(&mut host, act).await;

    assert_eq!(refused, Refusal::EntriesInFile);
}

#[tokio::test]
async fn a_born_file_with_entries_is_refused() {
    let mut host = host();
    let parent = || placed("/user/alice", "folder", Level::Public);
    let with_entries = create(
        parent(),
        born("readme", "note", None),
        vec![born("inside", "note", None)],
    );
    let bare = create(parent(), born("readme", "note", None), Vec::new());

    let refused = refusal(&mut host, with_entries).await;
    let made = settled(&mut host, bare).await;

    assert_eq!(refused, Refusal::EntriesInFile);
    let expected_made = vec![("readme".to_owned(), Level::Private)];
    assert_eq!(made, expected_made);
}

#[tokio::test]
async fn a_node_with_no_level_is_born_private() {
    let mut host = host();
    let parent = placed("/user/alice", "folder", Level::Public);
    let act = create(
        parent,
        born("works", "folder", None),
        vec![born("drafts", "folder", None), born("notes", "note", None)],
    );

    let made = settled(&mut host, act).await;

    let expected_made = vec![
        ("works".to_owned(), Level::Private),
        ("drafts".to_owned(), Level::Private),
        ("notes".to_owned(), Level::Private),
    ];
    assert_eq!(made, expected_made);
}

#[tokio::test]
async fn a_picked_level_is_kept_when_the_parent_allows_it() {
    let mut host = host();
    let parent = placed("/user/alice", "folder", Level::Public);
    let act = create(
        parent,
        born("works", "folder", Some(Level::Public)),
        vec![
            born("listed", "folder", Some(Level::Listed)),
            born("open", "note", Some(Level::Public)),
        ],
    );

    let made = settled(&mut host, act).await;

    let expected_made = vec![
        ("works".to_owned(), Level::Public),
        ("listed".to_owned(), Level::Listed),
        ("open".to_owned(), Level::Public),
    ];
    assert_eq!(made, expected_made);
}

#[tokio::test]
async fn a_node_wider_than_its_parent_is_refused() {
    let mut host = host();
    let cases = [
        (Level::Listed, Level::Public),
        (Level::Private, Level::Listed),
        (Level::Private, Level::Public),
    ];

    for (parent_level, picked) in cases {
        let parent = placed("/user/alice", "folder", parent_level);
        let act = create(parent, born("works", "folder", Some(picked)), Vec::new());

        let refused = refusal(&mut host, act).await;

        assert_eq!(
            refused,
            Refusal::WiderThanParent,
            "{picked:?} under {parent_level:?}"
        );
    }
}

#[tokio::test]
async fn an_entry_wider_than_its_node_is_refused() {
    let mut host = host();
    let picked_node = |level| {
        let parent = placed("/user/alice", "folder", Level::Public);
        let entries = vec![
            born("quiet", "folder", None),
            born("loud", "folder", Some(Level::Listed)),
        ];
        create(parent, born("works", "folder", level), entries)
    };

    // A node with no level is born Private, so a Listed entry is wider than it.
    let unpicked = refusal(&mut host, picked_node(None)).await;
    let private = refusal(&mut host, picked_node(Some(Level::Private))).await;

    assert_eq!(unpicked, Refusal::WiderThanParent);
    assert_eq!(private, Refusal::WiderThanParent);
}

#[tokio::test]
async fn depth_8_is_born_and_depth_9_is_refused() {
    let mut host = host();
    let at = |depth| placed(&path_at_depth(depth), "folder", Level::Public);
    let node_at_8 = create(at(7), born("eighth", "folder", None), Vec::new());
    let node_at_9 = create(at(8), born("ninth", "folder", None), Vec::new());
    let entry_at_8 = create(
        at(6),
        born("seventh", "folder", None),
        vec![born("eighth", "folder", None)],
    );
    let entry_at_9 = create(
        at(7),
        born("eighth", "folder", None),
        vec![born("ninth", "folder", None)],
    );

    let made_at_8 = settled(&mut host, node_at_8).await;
    let made_with_entry_at_8 = settled(&mut host, entry_at_8).await;
    let refused_at_9 = writer().act(&mut host, node_at_9).await;
    let refused_entry_at_9 = writer().act(&mut host, entry_at_9).await;

    assert_eq!(path_at_depth(8).matches('/').count(), 8);
    assert_eq!(made_at_8.len(), 1);
    assert_eq!(made_with_entry_at_8.len(), 2);
    assert!(matches!(
        refused_at_9,
        Err(WriteError::Refused(Refusal::TooDeep))
    ));
    assert!(matches!(
        refused_entry_at_9,
        Err(WriteError::Refused(Refusal::TooDeep))
    ));
    assert_eq!(host.applied().len(), 2, "a refused act reached the host");
}

#[tokio::test]
async fn an_act_on_a_node_of_another_type_is_refused() {
    let mut host = host();
    let commission = || placed("/commission/c1", "commission", Level::Public);
    let rename = act_of(
        "folder",
        Shape::Rename {
            node: commission(),
            name: DisplayName::host_checked("renamed".to_owned()),
        },
    );
    let change = act_of("folder", Shape::Change { node: commission() });
    let create_other = act_of(
        "folder",
        Shape::Create {
            parent: placed("/user/alice", "folder", Level::Public),
            node: born("works", "commission", None),
            entries: Vec::new(),
        },
    );
    let own_change = act_of("commission", Shape::Change { node: commission() });

    let renamed = refusal(&mut host, rename).await;
    let changed = refusal(&mut host, change).await;
    let created = refusal(&mut host, create_other).await;
    let own = writer().act(&mut host, own_change).await;

    assert_eq!(renamed, Refusal::OtherType);
    assert_eq!(changed, Refusal::OtherType);
    assert_eq!(created, Refusal::OtherType);
    assert!(own.is_ok(), "{own:?}");
}

#[tokio::test]
async fn two_entries_with_one_key_are_refused() {
    let mut host = host();
    let with_entries = |second: &str| {
        let parent = placed("/user/alice", "folder", Level::Public);
        let entries = vec![born("same", "folder", None), born(second, "note", None)];
        create(parent, born("works", "folder", None), entries)
    };

    let refused = refusal(&mut host, with_entries("same")).await;
    let made = settled(&mut host, with_entries("other")).await;

    assert_eq!(refused, Refusal::DuplicateKey);
    assert_eq!(made.len(), 3);
}

#[tokio::test]
async fn a_refused_act_never_calls_apply() {
    let at = |depth| placed(&path_at_depth(depth), "folder", Level::Public);
    let refused_acts = [
        create(
            placed("/user/alice/readme", "note", Level::Public),
            born("inside", "folder", None),
            Vec::new(),
        ),
        create(
            at(2),
            born("works", "folder", Some(Level::Public)),
            Vec::new(),
        )
        .narrowed(),
        create(at(8), born("ninth", "folder", None), Vec::new()),
        act_of("folder", Shape::Change { node: at(3) }).retyped("commission"),
        create(
            at(2),
            born("works", "folder", None),
            vec![born("same", "folder", None), born("same", "folder", None)],
        ),
    ];

    for act in refused_acts {
        let mut host = host();

        let written = writer().act(&mut host, act).await;

        assert!(
            matches!(written, Err(WriteError::Refused(_))),
            "{written:?}"
        );
        assert!(host.applied().is_empty(), "{written:?} reached the host");
        assert!(host.calls().is_empty(), "{written:?} called the host");
    }
}

// The dry run answers each act as `act` would, and leaves the host as it found it: no apply,
// no call.
#[tokio::test]
async fn the_dry_run_refuses_what_act_refuses_and_applies_nothing() {
    let at = |depth| placed(&path_at_depth(depth), "folder", Level::Public);
    let acts = || {
        [
            create(
                placed("/user/alice/readme", "note", Level::Public),
                born("inside", "folder", None),
                Vec::new(),
            ),
            create(
                at(2),
                born("works", "folder", Some(Level::Public)),
                Vec::new(),
            )
            .narrowed(),
            create(at(8), born("ninth", "folder", None), Vec::new()),
            act_of("folder", Shape::Change { node: at(3) }).retyped("commission"),
            create(
                at(2),
                born("works", "folder", None),
                vec![born("same", "folder", None), born("same", "folder", None)],
            ),
            create(at(2), born("works", "folder", None), Vec::new()),
            act_of("folder", Shape::Change { node: at(3) }),
        ]
    };

    for (dry, real) in acts().into_iter().zip(acts()) {
        let mut host = host();

        let dry_run = writer().dry_run(&host, &dry);

        assert!(host.applied().is_empty(), "{dry:?}: the dry run applied it");
        assert!(
            host.calls().is_empty(),
            "{dry:?}: the dry run called the host"
        );
        let written = writer().act(&mut host, real).await;
        let acted = match written {
            Ok(()) => Ok(()),
            Err(WriteError::Refused(refusal)) => Err(refusal),
            Err(WriteError::Host(_)) => panic!("{dry:?}: the fake host failed"),
        };
        assert_eq!(dry_run, acted, "{dry:?}");
    }
}

impl Act<()> {
    /// This act with its parent narrowed to Private.
    fn narrowed(mut self) -> Self {
        if let Shape::Create { parent, .. } = &mut self.shape {
            parent.level = Level::Private;
        }
        self
    }

    /// This act with its node's type changed to `typ`.
    fn retyped(mut self, typ: &str) -> Self {
        if let Shape::Change { node } = &mut self.shape {
            node.typ = Type::from(typ.to_owned());
        }
        self
    }
}

#[tokio::test]
async fn a_key_of_percent_2e_percent_2e_is_born_as_written() {
    let mut host = host();
    let parent = placed("/user/alice", "folder", Level::Public);
    let act = create(
        parent,
        born("%2e%2e", "folder", None),
        vec![born("%2E%2e%2f", "note", None)],
    );

    let made = settled(&mut host, act).await;

    let expected_made = vec![
        ("%2e%2e".to_owned(), Level::Private),
        ("%2E%2e%2f".to_owned(), Level::Private),
    ];
    assert_eq!(made, expected_made);
}

#[tokio::test]
async fn debug_prints_no_viewer_path_segment_or_name() {
    let mut host = host();
    let parent = placed("/user/planted-parent", "folder", Level::Public);
    let node = Born {
        name: DisplayName::host_checked("Planted Node Name".to_owned()),
        ..born("planted-node-key", "folder", Some(Level::Listed))
    };
    let entry = Born {
        name: DisplayName::host_checked("Planted Entry Name".to_owned()),
        ..born("planted-entry-key", "note", None)
    };
    let creation = create(parent.clone(), node.clone(), vec![entry.clone()]);
    let rename = act_of(
        "folder",
        Shape::Rename {
            node: parent.clone(),
            name: DisplayName::host_checked("Planted New Name".to_owned()),
        },
    );
    let change = act_of(
        "folder",
        Shape::Change {
            node: parent.clone(),
        },
    );
    writer()
        .act(&mut host, creation.clone())
        .await
        .expect("the act applies");
    let checked = host.applied().last().expect("the act was applied");
    let host_failure: Box<dyn std::error::Error + Send + Sync> =
        "planted-parent is down: Planted Node Name".into();
    let host_error = WriteError::Host(HostError::from(host_failure));
    let refused = WriteError::Refused(Refusal::TooDeep);

    let printed = [
        format!("{:?}", writer()),
        format!("{creation:?}"),
        format!("{rename:?}"),
        format!("{change:?}"),
        format!("{:?}", rename.shape),
        format!("{parent:?}"),
        format!("{node:?}"),
        format!("{entry:?}"),
        format!("{checked:?}"),
        format!("{host_error:?}"),
        format!("{refused:?}"),
        format!("{:?}", Refusal::OtherType),
    ];

    for text in &printed {
        for planted in [
            VIEWER,
            "viewer-planted",
            "planted-parent",
            "planted-node-key",
            "planted-entry-key",
            "Planted",
            "name of",
        ] {
            assert!(!text.contains(planted), "{planted} printed in {text}");
        }
    }
    assert_eq!(format!("{refused:?}"), "Refused(too_deep)");
    assert!(printed[1].contains("create") && printed[1].contains("depth: 2"));
}

/// A host that keeps the default [`Host::apply`].
struct ReadOnlyHost;

#[async_trait::async_trait]
impl Host for ReadOnlyHost {
    type Act = ();
    type Applied = ();

    async fn child(
        &self,
        _viewer: &Viewer,
        _parent: &RealPath,
        _key: &Segment,
    ) -> Result<Option<Found>, HostError> {
        Ok(None)
    }

    async fn children(
        &self,
        _viewer: &Viewer,
        _parent: &RealPath,
    ) -> Result<Vec<Found>, HostError> {
        Ok(Vec::new())
    }

    async fn mounts(&self, _viewer: &Viewer, _folder: &RealPath) -> Result<Vec<Mount>, HostError> {
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
        Admission::Refuse
    }
}

#[tokio::test]
async fn the_default_apply_refuses_every_checked_act() {
    let mut host = ReadOnlyHost;
    let parent = placed("/user/alice", "folder", Level::Public);
    let act = create(parent, born("works", "folder", None), Vec::new());

    let written = writer().act(&mut host, act).await;

    assert!(matches!(written, Err(WriteError::Host(_))), "{written:?}");
}

#[tokio::test]
async fn a_failing_host_is_a_host_error() {
    let mut host = host().failing();
    let parent = placed("/user/alice", "folder", Level::Public);
    let act = create(parent, born("works", "folder", None), Vec::new());

    let written = writer().act(&mut host, act).await;

    assert!(matches!(written, Err(WriteError::Host(_))), "{written:?}");
}

#[test]
fn an_act_is_a_send_future() {
    fn send<F: std::future::Future + Send>(_future: F) {}
    let mut host = host();
    let parent = placed("/user/alice", "folder", Level::Public);
    let act = create(parent, born("works", "folder", None), Vec::new());

    send(writer().act(&mut host, act));
}
