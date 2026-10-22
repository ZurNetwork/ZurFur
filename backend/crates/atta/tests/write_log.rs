//! What the writer logs: each act names its outcome class, its word and its depth at debug,
//! never the viewer, a path, a segment or a name. Alone in its binary: tracing caches a
//! callsite's interest, so a sibling test hitting this callsite first could blind the capture.

use std::sync::{Arc, Mutex};

use atta::{
    Access, Act, ActName, Admission, Born, Caps, Checked, DisplayName, Found, Host, HostError,
    Kind, Level, Metadata, Mount, Placed, Reach, RealPath, Segment, Shape, Type, Viewer, Writer,
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

/// A host whose every type is a directory, that applies each act or, when `failing`, fails
/// with a message naming the node.
struct ActHost {
    failing: bool,
}

#[async_trait::async_trait]
impl Host for ActHost {
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
        Admission::Admit
    }

    async fn apply(&mut self, _viewer: &Viewer, _checked: Checked<()>) -> Result<(), HostError> {
        if self.failing {
            let failure: Box<dyn std::error::Error + Send + Sync> =
                "the store is down at alice-root, Hidden Works".into();
            return Err(HostError::from(failure));
        }
        Ok(())
    }
}

fn segment(text: &str) -> Segment {
    Segment::try_from(text).expect("a valid test segment")
}

/// A Public folder at `pieces`.
fn folder(pieces: &[&str]) -> Placed {
    Placed {
        path: pieces.iter().map(|piece| segment(piece)).collect(),
        typ: Type::from("folder".to_owned()),
        level: Level::Public,
    }
}

/// An act named `word` making the folder `key`, named "Hidden Works", at `level` in `parent`.
fn make(word: &str, parent: Placed, key: &str, level: Option<Level>) -> Act<()> {
    let node = Born {
        key: segment(key),
        typ: Type::from("folder".to_owned()),
        name: DisplayName::host_checked("Hidden Works".to_owned()),
        level,
    };
    let shape = Shape::Create {
        parent,
        node,
        entries: Vec::new(),
    };
    Act {
        name: ActName::from(word.to_owned()),
        typ: Type::from("folder".to_owned()),
        shape,
        body: (),
    }
}

fn writer() -> Writer {
    Writer {
        viewer: Viewer::from("did:plc:alice-viewer".to_owned()),
        caps: Caps::default(),
    }
}

/// The line logging `outcome` for the act `word`, among `text`'s.
fn outcome_line<'t>(text: &'t str, outcome: &str, word: &str) -> Option<&'t str> {
    let outcome_field = format!("outcome=\"{outcome}\"");
    let act_field = format!("act=\"{word}\"");
    text.lines().find(|line| {
        line.contains("acted")
            && line.contains(outcome_field.as_str())
            && line.contains(act_field.as_str())
    })
}

#[tokio::test]
async fn each_act_logs_its_outcome_class_word_and_depth_and_never_a_path_or_name() {
    let mut host = ActHost { failing: false };
    let mut failing = ActHost { failing: true };
    let captured = Captured::default();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(captured.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let root = || folder(&["user", "alice-root"]);
    let narrow_root = Placed {
        level: Level::Private,
        ..root()
    };

    let applied = writer()
        .act(&mut host, make("make_folder", root(), "secret-key", None))
        .await;
    let wider = writer()
        .act(
            &mut host,
            make(
                "open_folder",
                narrow_root,
                "secret-key",
                Some(Level::Public),
            ),
        )
        .await;
    let broken = writer()
        .act(
            &mut failing,
            make("break_folder", root(), "secret-key", None),
        )
        .await;

    assert!(applied.is_ok() && wider.is_err() && broken.is_err());
    let text = captured.text();
    let applied_line =
        outcome_line(&text, "applied", "make_folder").unwrap_or_else(|| panic!("{text}"));
    assert!(
        applied_line.contains("depth=3") && applied_line.contains("DEBUG"),
        "{applied_line}"
    );
    let refused_line =
        outcome_line(&text, "refused", "open_folder").unwrap_or_else(|| panic!("{text}"));
    assert!(
        refused_line.contains("depth=3") && refused_line.contains("refusal=\"wider_than_parent\""),
        "{refused_line}"
    );
    let host_error_line =
        outcome_line(&text, "host_error", "break_folder").unwrap_or_else(|| panic!("{text}"));
    assert!(host_error_line.contains("depth=3"), "{host_error_line}");
    for planted in [
        "alice",
        "secret-key",
        "Hidden Works",
        "store is down",
        "user",
    ] {
        assert!(!text.contains(planted), "the log carries {planted}: {text}");
    }
}
