//! The Den's golden responses under `contract/golden/den/`: the served den routes' answers on
//! the in-memory fixture world, signed in as alice and as bob, plus hand-written card fixtures
//! built through the generated types, each file in one canonical JSON form and listed in an
//! index. With `DEN_GOLDEN=write` the run rewrites them; without it, it fails on any drift.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use adapter_mem::fixture::{
    alice, bob, ember, old_sketch, queue, seed_commission, seed_identity, supreme_arts, untitled,
    untitled_files, untitled_slots,
};
use api::generated::{
    DenCard, DenNode, GetDenNodeResponse, GetDenRootResponse, Problem, den_node,
    get_den_node_response,
};
use domain::elements::{profile::Profile, user::UserId};
use serde_json::Value;
use test_support::http::{client, serve, sign_in};

/// The golden folder, from this crate's manifest directory.
const GOLDEN_DIR: &str = "../../../contract/golden/den";

/// The index of every golden file, inside the golden folder.
const INDEX_FILE: &str = "index.json";

/// The environment switch that makes the run rewrite the files instead of checking them.
const WRITE_SWITCH: &str = "DEN_GOLDEN";

/// The switch's one value that writes.
const WRITE_VALUE: &str = "write";

/// The recipe that regenerates the files.
const RECIPE: &str = "just den-golden";

/// The one method the den routes serve.
const GET: &str = "GET";

/// Who a golden answer was read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
enum Viewer {
    Alice,
    Bob,
}

impl Viewer {
    /// The fixture user behind this viewer.
    fn user(self) -> UserId {
        match self {
            Self::Alice => alice(),
            Self::Bob => bob(),
        }
    }

    /// The handle the viewer signs in with.
    fn handle(self) -> &'static str {
        match self {
            Self::Alice => "alice.test",
            Self::Bob => "bob.test",
        }
    }

    /// The profile the fake PDS answers for the viewer, the one the fixture world caches.
    fn profile(self) -> Profile {
        let profile = Profile::new(self.user().did().clone(), self.handle());
        match self {
            Self::Alice => profile.with_display_name("Alice"),
            Self::Bob => profile,
        }
    }
}

/// Where a golden file's content came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Source {
    /// Recorded from a served route: regenerated, never patched.
    Generated,
    /// Built by hand through the generated types: no fixture-world answer carries it.
    Handwritten,
}

/// The contract message a golden body decodes as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, strum::Display)]
enum Message {
    GetDenRootResponse,
    GetDenNodeResponse,
    Problem,
}

impl Message {
    /// Asserts `wire` decodes as this message and re-encodes to the same bytes, so a golden
    /// body is exactly the generated type's encoding.
    fn assert_round_trips(self, wire: &[u8], file: &str) {
        let reencoded = match self {
            Self::GetDenRootResponse => reencode::<GetDenRootResponse>(wire, file),
            Self::GetDenNodeResponse => reencode::<GetDenNodeResponse>(wire, file),
            Self::Problem => reencode::<Problem>(wire, file),
        };
        assert_eq!(
            String::from_utf8_lossy(&reencoded),
            String::from_utf8_lossy(wire),
            "{file}: the wire body is not the generated type's own encoding"
        );
    }
}

/// `wire` decoded as `T`, encoded again.
fn reencode<T: serde::Serialize + serde::de::DeserializeOwned>(wire: &[u8], file: &str) -> Vec<u8> {
    let decoded: T = serde_json::from_slice(wire)
        .unwrap_or_else(|error| panic!("{file}: the body doesn't decode: {error}"));
    serde_json::to_vec(&decoded).expect("a generated message encodes")
}

/// One golden file's line in the index.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct IndexEntry {
    /// The file, relative to the golden folder.
    file: String,
    source: Source,
    viewer: Viewer,
    method: &'static str,
    /// The API's own path and query, as the web's upstream client requests it.
    path: String,
    status: u16,
    content_type: String,
    message: Message,
}

/// One golden file: its index line and its body.
#[derive(Clone, Debug, PartialEq)]
struct Golden {
    entry: IndexEntry,
    body: Value,
}

/// One request to record, and the answer it must get.
struct Case {
    file: &'static str,
    viewer: Viewer,
    path: String,
    status: u16,
    message: Message,
}

impl Case {
    /// A `200` answer of `message`.
    fn ok(file: &'static str, viewer: Viewer, path: String, message: Message) -> Self {
        Self {
            file,
            viewer,
            path,
            status: 200,
            message,
        }
    }

    /// A `200` node answer.
    fn node(file: &'static str, viewer: Viewer, path: String) -> Self {
        Self::ok(file, viewer, path, Message::GetDenNodeResponse)
    }

    /// A problem answer with `status`.
    fn problem(file: &'static str, viewer: Viewer, path: &str, status: u16) -> Self {
        Self {
            file,
            viewer,
            path: path.to_owned(),
            status,
            message: Message::Problem,
        }
    }
}

/// Every request the generated files record, in index order.
fn cases() -> Vec<Case> {
    use Viewer::{Alice, Bob};
    let supreme_arts = format!("/den/accounts/{}", supreme_arts());
    let untitled = format!("/den/commissions/{}", untitled());
    let [ref_abco, _] = untitled_files();
    let [abco_slot, ..] = untitled_slots();
    vec![
        Case::ok(
            "alice/root.json",
            Alice,
            "/den".to_owned(),
            Message::GetDenRootResponse,
        ),
        Case::node("alice/accounts.json", Alice, "/den/accounts".to_owned()),
        Case::node("alice/supreme-arts.json", Alice, supreme_arts.clone()),
        Case::node(
            "alice/supreme-arts-characters.json",
            Alice,
            format!("{supreme_arts}/characters"),
        ),
        Case::node(
            "alice/supreme-arts-commissions.json",
            Alice,
            format!("{supreme_arts}/commissions"),
        ),
        Case::node(
            "alice/supreme-arts-workflows.json",
            Alice,
            format!("{supreme_arts}/workflows"),
        ),
        Case::node(
            "alice/supreme-arts-queue.json",
            Alice,
            format!("{supreme_arts}/workflows/{}", queue()),
        ),
        Case::node("alice/characters.json", Alice, "/den/characters".to_owned()),
        Case::node(
            "alice/ember.json",
            Alice,
            format!("/den/characters/{}", ember()),
        ),
        Case::node(
            "alice/commissions.json",
            Alice,
            "/den/commissions".to_owned(),
        ),
        Case::node(
            "alice/commissions-include-deleted.json",
            Alice,
            "/den/commissions?includeDeleted=true".to_owned(),
        ),
        Case::node("alice/untitled.json", Alice, untitled.clone()),
        Case::node(
            "alice/untitled-attachments.json",
            Alice,
            format!("{untitled}/attachments"),
        ),
        Case::node(
            "alice/untitled-attachment-ref-abco.json",
            Alice,
            format!("{untitled}/attachments/{ref_abco}"),
        ),
        Case::node(
            "alice/untitled-changelog.json",
            Alice,
            format!("{untitled}/changelog"),
        ),
        Case::node(
            "alice/untitled-products.json",
            Alice,
            format!("{untitled}/products"),
        ),
        Case::node(
            "alice/untitled-slots.json",
            Alice,
            format!("{untitled}/slots"),
        ),
        Case::node(
            "alice/untitled-slot-abco.json",
            Alice,
            format!("{untitled}/slots/{abco_slot}"),
        ),
        Case::node(
            "alice/old-sketch.json",
            Alice,
            format!("/den/commissions/{}", old_sketch()),
        ),
        Case::problem("alice/not-found.json", Alice, "/den/nope", 404),
        Case::problem(
            "alice/unknown-parameter.json",
            Alice,
            "/den/accounts?nope=1",
            400,
        ),
        Case::problem(
            "alice/invalid-request.json",
            Alice,
            "/den/accounts?includeDeleted=yes",
            422,
        ),
        Case::ok(
            "bob/root.json",
            Bob,
            "/den".to_owned(),
            Message::GetDenRootResponse,
        ),
        Case::node("bob/accounts.json", Bob, "/den/accounts".to_owned()),
        Case::node("bob/characters.json", Bob, "/den/characters".to_owned()),
        Case::node("bob/commissions.json", Bob, "/den/commissions".to_owned()),
    ]
}

/// A served fixture world with a client signed in as `viewer`.
struct Session {
    viewer: Viewer,
    base_url: String,
    client: reqwest::Client,
}

impl Session {
    /// Serves a fresh fixture world and signs `viewer` in.
    async fn open(viewer: Viewer) -> Self {
        let builder = test_support::runtime::mem(viewer.user().did()).profile(viewer.profile());
        let served = serve(builder, api::app).await;
        seed_identity(&served.backend)
            .await
            .expect("seeds identity's fixtures");
        seed_commission(&served.backend, &alice(), &bob(), &supreme_arts())
            .await
            .expect("seeds commission's fixtures");
        let client = client();
        sign_in(&client, &served.base_url, viewer.handle()).await;
        Self {
            viewer,
            base_url: served.base_url,
            client,
        }
    }

    /// Requests `case` and records its answer, after checking its status and its encoding.
    async fn record(&self, case: Case) -> Golden {
        assert_eq!(case.viewer, self.viewer, "{}", case.file);
        let response = self
            .client
            .get(format!("{}{}", self.base_url, case.path))
            .send()
            .await
            .expect("the request is answered");
        let status = response.status().as_u16();
        assert_eq!(status, case.status, "{}: {}", case.file, case.path);
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .expect("a Content-Type header")
            .to_str()
            .expect("an ASCII header")
            .to_owned();
        let wire = response.bytes().await.expect("a body");
        case.message.assert_round_trips(&wire, case.file);
        let body: Value = serde_json::from_slice(&wire).expect("a JSON body");
        let entry = IndexEntry {
            file: case.file.to_owned(),
            source: Source::Generated,
            viewer: case.viewer,
            method: GET,
            path: case.path,
            status,
            content_type,
            message: case.message,
        };
        Golden { entry, body }
    }
}

/// Every generated golden, read from two fresh fixture worlds, one per viewer.
async fn generated() -> Vec<Golden> {
    let alice_session = Session::open(Viewer::Alice).await;
    let bob_session = Session::open(Viewer::Bob).await;
    let mut goldens = Vec::new();
    for case in cases() {
        let session = match case.viewer {
            Viewer::Alice => &alice_session,
            Viewer::Bob => &bob_session,
        };
        goldens.push(session.record(case).await);
    }
    goldens
}

/// The generated golden recorded into `file`.
fn recorded<'a>(goldens: &'a [Golden], file: &str) -> &'a Golden {
    goldens
        .iter()
        .find(|golden| golden.entry.file == file)
        .unwrap_or_else(|| panic!("no generated golden {file}"))
}

/// A recorded node answer, decoded into the generated type.
fn node_response(golden: &Golden) -> GetDenNodeResponse {
    serde_json::from_value(golden.body.clone()).expect("a node answer")
}

/// A card entry: only its name, its type and, on the viewer's own mount, the mount mark.
fn card(name: &str, den_type: &str, mount: bool) -> DenNode {
    DenNode {
        segments: Vec::new(),
        name: name.to_owned(),
        r#type: den_type.to_owned(),
        kind: String::new(),
        mount,
        access: Some(den_node::Access::Card(DenCard {})),
    }
}

/// `response`'s listing with `entry` added in name order, as the server orders entries.
fn with_entry(mut response: GetDenNodeResponse, entry: DenNode) -> GetDenNodeResponse {
    let Some(get_den_node_response::Content::Listing(listing)) = response.content.as_mut() else {
        panic!("a card fixture is built over an open listing");
    };
    listing.entries.push(entry);
    listing
        .entries
        .sort_by(|left, right| left.name.cmp(&right.name));
    response
}

/// A hand-written golden of `response`, standing for alice's `GET path`.
fn handwritten(file: &str, path: &str, response: &GetDenNodeResponse) -> Golden {
    let entry = IndexEntry {
        file: file.to_owned(),
        source: Source::Handwritten,
        viewer: Viewer::Alice,
        method: GET,
        path: path.to_owned(),
        status: 200,
        content_type: "application/json".to_owned(),
        message: Message::GetDenNodeResponse,
    };
    let body = serde_json::to_value(response).expect("a generated message encodes");
    Golden { entry, body }
}

/// The card fixtures no 0.1 data produces, built over the generated answers they extend.
fn handwritten_cards(generated: &[Golden]) -> Vec<Golden> {
    let commissions = node_response(recorded(generated, "alice/commissions.json"));
    let accounts = node_response(recorded(generated, "alice/accounts.json"));
    let untitled = node_response(recorded(generated, "alice/untitled.json"));

    let hidden_commission = card("Hidden commission", "commission", false);
    let locked_studio = card("Locked Studio", "account", true);
    let listing_with_card = with_entry(commissions, hidden_commission.clone());
    let listing_with_card_mount = with_entry(accounts, locked_studio);
    let card_node = GetDenNodeResponse {
        node: Some(hidden_commission),
        crumbs: untitled.crumbs,
        content: None,
    };
    vec![
        handwritten(
            "handwritten/listing-with-card.json",
            "/den/commissions",
            &listing_with_card,
        ),
        handwritten(
            "handwritten/listing-with-card-mount.json",
            "/den/accounts",
            &listing_with_card_mount,
        ),
        handwritten(
            "handwritten/card-node.json",
            "/den/commissions/hidden-commission",
            &card_node,
        ),
    ]
}

/// `value` with every object's keys in sorted order, whatever map order serde_json keeps.
fn sorted_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut members: Vec<(String, Value)> = map.into_iter().collect();
            members.sort_by(|left, right| left.0.cmp(&right.0));
            let sorted = members
                .into_iter()
                .map(|(key, member)| (key, sorted_keys(member)))
                .collect();
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted_keys).collect()),
        scalar => scalar,
    }
}

/// The canonical text of `value`: keys sorted, pretty-printed, newline-terminated.
fn canonical(value: &Value) -> String {
    let sorted = sorted_keys(value.clone());
    let mut text = serde_json::to_string_pretty(&sorted).expect("JSON prints");
    text.push('\n');
    text
}

/// Every golden file's text by its path in the golden folder, the index included.
fn rendered(goldens: &[Golden]) -> BTreeMap<PathBuf, String> {
    let entries: Vec<&IndexEntry> = goldens.iter().map(|golden| &golden.entry).collect();
    let index = serde_json::json!({ "files": entries });
    let mut files: BTreeMap<PathBuf, String> = goldens
        .iter()
        .map(|golden| (PathBuf::from(&golden.entry.file), canonical(&golden.body)))
        .collect();
    files.insert(PathBuf::from(INDEX_FILE), canonical(&index));
    files
}

/// Every golden file, generated and hand-written, rendered once.
async fn generation() -> BTreeMap<PathBuf, String> {
    let mut goldens = generated().await;
    let cards = handwritten_cards(&goldens);
    goldens.extend(cards);
    rendered(&goldens)
}

/// The golden folder.
fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN_DIR)
}

/// Every `.json` file under `directory`, by its path relative to `root`.
fn committed_json(root: &Path, directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            files.extend(committed_json(root, &path));
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let relative = path.strip_prefix(root).expect("under the root").to_owned();
            files.push(relative);
        }
    }
    files
}

/// Whether this run rewrites the files.
fn write_requested() -> bool {
    std::env::var(WRITE_SWITCH).is_ok_and(|value| value == WRITE_VALUE)
}

/// Rewrites the golden folder's JSON to exactly `files`, removing any file the generation
/// no longer makes.
fn write(files: &BTreeMap<PathBuf, String>) {
    let root = golden_dir();
    for stale in committed_json(&root, &root) {
        if !files.contains_key(&stale) {
            std::fs::remove_file(root.join(&stale)).expect("removes a stale golden");
        }
    }
    for (file, text) in files {
        let path = root.join(file);
        let parent = path.parent().expect("a golden file has a folder");
        std::fs::create_dir_all(parent).expect("creates the golden folder");
        std::fs::write(&path, text).expect("writes a golden");
    }
}

/// Every way the committed folder differs from `files`, one line each.
fn drift(files: &BTreeMap<PathBuf, String>) -> Vec<String> {
    let root = golden_dir();
    let committed = committed_json(&root, &root);
    let mut differences = Vec::new();
    for file in &committed {
        if !files.contains_key(file) {
            differences.push(format!(
                "{}: committed, but no longer generated",
                file.display()
            ));
        }
    }
    for (file, text) in files {
        let on_disk = std::fs::read_to_string(root.join(file));
        match on_disk {
            Err(_) => differences.push(format!("{}: generated, but not committed", file.display())),
            Ok(committed_text) if committed_text != *text => {
                differences.push(format!(
                    "{}: differs from a fresh generation",
                    file.display()
                ));
            }
            Ok(_) => {}
        }
    }
    differences
}

#[tokio::test]
async fn the_committed_golden_responses_are_a_fresh_generation() {
    let first = generation().await;
    let second = generation().await;
    assert_eq!(
        first, second,
        "two generations differ: something time-dependent or random reached a golden file"
    );

    if write_requested() {
        write(&first);
        return;
    }
    let differences = drift(&first);
    assert!(
        differences.is_empty(),
        "the golden responses under contract/golden/den drifted; regenerate them with `{RECIPE}`, \
         never patch them:\n{}",
        differences.join("\n")
    );
}

#[test]
fn canonical_json_sorts_keys_at_every_depth_and_ends_in_a_newline() {
    let value = serde_json::json!({ "b": [{ "z": 1, "a": 2 }], "a": "x" });
    let expected =
        "{\n  \"a\": \"x\",\n  \"b\": [\n    {\n      \"a\": 2,\n      \"z\": 1\n    }\n  ]\n}\n";

    let text = canonical(&value);

    assert_eq!(text, expected);
}
