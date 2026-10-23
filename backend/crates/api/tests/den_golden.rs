//! The Den's golden responses under `contract/golden/den/`: the served den routes' answers on
//! the in-memory fixture world, signed in as alice and as bob, plus hand-written card fixtures
//! built through the generated types, each file in one canonical JSON form and listed in an
//! index. With `DEN_GOLDEN=write` the run rewrites them; otherwise it fails on any drift.

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
use domain::elements::profile::Profile;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Viewer {
    Alice,
    Bob,
    /// No one signed in.
    Anonymous,
}

impl Viewer {
    /// The handle a signed-in viewer signs in with.
    fn handle(self) -> Option<&'static str> {
        match self {
            Self::Alice => Some("alice.test"),
            Self::Bob => Some("bob.test"),
            Self::Anonymous => None,
        }
    }

    /// The profile the fake PDS answers for a signed-in viewer, the one the fixture world caches.
    fn profile(self) -> Option<Profile> {
        match self {
            Self::Alice => {
                let profile = Profile::new(alice().did().clone(), "alice.test");
                Some(profile.with_display_name("Alice"))
            }
            Self::Bob => Some(Profile::new(bob().did().clone(), "bob.test")),
            Self::Anonymous => None,
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
#[derive(Clone, Copy, Debug, serde::Serialize)]
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
#[derive(Debug, serde::Serialize)]
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
#[derive(Debug)]
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
    use Viewer::{Alice, Anonymous, Bob};
    let supreme_arts = format!("/den/accounts/{}", supreme_arts());
    let untitled = format!("/den/commissions/{}", untitled());
    let [ref_abco, _] = untitled_files();
    let [abco_slot, ..] = untitled_slots();
    let cases = vec![
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
        Case::node("alice/posts.json", Alice, "/den/posts".to_owned()),
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
        Case::problem("anonymous/not-authenticated.json", Anonymous, "/den", 401),
    ];
    cases
}

/// A served fixture world with a client signed in as `viewer`, or not signed in at all.
struct Session {
    viewer: Viewer,
    base_url: String,
    client: reqwest::Client,
}

impl Session {
    /// Serves a fresh fixture world and signs `viewer` in, if anyone.
    async fn open(viewer: Viewer) -> Self {
        let builder = match viewer.profile() {
            Some(profile) => test_support::runtime::mem(&profile.did).profile(profile),
            // No one signs in, so the fake PDS is never asked.
            None => test_support::runtime::mem(alice().did()),
        };
        let served = serve(builder, api::app).await;
        seed_identity(&served.backend)
            .await
            .expect("seeds identity's fixtures");
        seed_commission(&served.backend, &alice(), &bob(), &supreme_arts())
            .await
            .expect("seeds commission's fixtures");
        let client = client();
        if let Some(handle) = viewer.handle() {
            sign_in(&client, &served.base_url, handle).await;
        }
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

/// Every generated golden, read from fresh fixture worlds, one per viewer.
async fn generated() -> Vec<Golden> {
    let alice_session = Session::open(Viewer::Alice).await;
    let bob_session = Session::open(Viewer::Bob).await;
    let anonymous_session = Session::open(Viewer::Anonymous).await;
    let mut goldens = Vec::new();
    for case in cases() {
        let session = match case.viewer {
            Viewer::Alice => &alice_session,
            Viewer::Bob => &bob_session,
            Viewer::Anonymous => &anonymous_session,
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
        name_ordinal: None,
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
    let cards = vec![
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
    ];
    cards
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

/// Every `.json` file under `root`, by its path relative to it.
fn committed_json(root: &Path) -> Vec<PathBuf> {
    json_under(root, root)
}

/// Every `.json` file under `directory`, by its path relative to `root`.
fn json_under(root: &Path, directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries {
        let path = entry.expect("a directory entry").path();
        if path.is_dir() {
            files.extend(json_under(root, &path));
        } else if path
            .extension()
            .is_some_and(|extension| extension == "json")
        {
            let relative = path.strip_prefix(root).expect("under the root").to_owned();
            files.push(relative);
        }
    }
    files.sort();
    files
}

/// Whether the write switch's value asks the run to rewrite the files: only `write` does.
fn writes(switch: Option<&str>) -> bool {
    switch == Some(WRITE_VALUE)
}

/// Rewrites the JSON under `root` to exactly `files`, removing any file the generation no
/// longer makes.
fn write(root: &Path, files: &BTreeMap<PathBuf, String>) {
    for stale in committed_json(root) {
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

/// Every way the JSON under `root` differs from `files`, one line each.
fn drift(root: &Path, files: &BTreeMap<PathBuf, String>) -> Vec<String> {
    let mut differences = Vec::new();
    for file in committed_json(root) {
        if !files.contains_key(&file) {
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

    let switch = std::env::var(WRITE_SWITCH).ok();
    if writes(switch.as_deref()) {
        write(&golden_dir(), &first);
        return;
    }
    let differences = drift(&golden_dir(), &first);
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

#[test]
fn only_the_value_write_turns_the_write_switch_on() {
    let switches = [
        None,
        Some("write"),
        Some("1"),
        Some("WRITE"),
        Some("check"),
        Some(""),
    ];

    let writing: Vec<bool> = switches.into_iter().map(writes).collect();

    assert_eq!(writing, [false, true, false, false, false, false]);
}

/// A generation of two files, one in a subfolder.
fn two_files() -> BTreeMap<PathBuf, String> {
    let files = [
        (PathBuf::from("index.json"), "{}\n".to_owned()),
        (PathBuf::from("alice/root.json"), "[]\n".to_owned()),
    ];
    BTreeMap::from(files)
}

#[test]
fn a_folder_written_from_a_generation_has_no_drift() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let files = two_files();

    write(folder.path(), &files);

    assert!(drift(folder.path(), &files).is_empty());
}

#[test]
fn drift_names_a_changed_a_missing_and_an_extra_file() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let files = two_files();
    write(folder.path(), &files);
    std::fs::write(folder.path().join("index.json"), "{ }\n").expect("changes a file");
    std::fs::remove_file(folder.path().join("alice/root.json")).expect("removes a file");
    std::fs::create_dir_all(folder.path().join("bob")).expect("creates a subfolder");
    std::fs::write(folder.path().join("bob/extra.json"), "{}\n").expect("adds a file");
    let expected = [
        "bob/extra.json: committed, but no longer generated",
        "alice/root.json: generated, but not committed",
        "index.json: differs from a fresh generation",
    ];

    let differences = drift(folder.path(), &files);

    assert_eq!(differences, expected);
}

#[test]
fn write_removes_a_stale_file_and_keeps_what_isn_t_json() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    std::fs::create_dir_all(folder.path().join("bob")).expect("creates a subfolder");
    std::fs::write(folder.path().join("bob/stale.json"), "{}\n").expect("adds a stale file");
    std::fs::write(folder.path().join("README.md"), "# kept\n").expect("adds a README");
    let files = two_files();

    write(folder.path(), &files);

    let expected = [
        PathBuf::from("alice/root.json"),
        PathBuf::from("index.json"),
    ];
    assert_eq!(committed_json(folder.path()), expected);
    assert!(folder.path().join("README.md").exists());
}
