//! `GET /den` and `GET /den/{*path}` on the PostgreSQL stores, over the den world seeded
//! through the app's routes: alice's root, her membership mount, the archived "Old sketch",
//! and the one 404, byte for byte, for every cause, the stranger's and the Participant's
//! pairs of causes included; and `scripts/den-fill.sh` filling a signed-in user's Den
//! through the routes. Requires a container runtime socket, and the `docker` CLI, `bash`,
//! `curl` and `jq` for the fill script.

use std::{process::Stdio, sync::Arc};

use domain::elements::{did::Did, profile::Profile};
use serde_json::{Value, json};
use test_support::{
    den::{Answer, PRIVATE_NO_STORE, entry_names, get, ok_json},
    pg_app::{
        self, DenWorld, MissPair, SignedIn, UNNAMED, absent_commission, den_participant_pairs,
        den_stranger_pairs, den_stranger_targets,
    },
    recording::{CallLog, RecordingCommissionStore, RecordingWorkflowStore},
};
use tokio::{io::AsyncWriteExt as _, process::Command};

/// The fill script, from the repository's root.
const DEN_FILL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../scripts/den-fill.sh");

/// The fill script's user: a DID no other fixture uses.
const FILLER_DID: &str = "did:plc:denfillfixtureaaaaaaaaaa";

/// Asserts alice opens `pair`'s real node at its home, so the pair never compares two
/// absent nodes.
async fn assert_alice_opens_its_home(world: &DenWorld, pair: &MissPair) {
    let home = get(&world.alice, world.base_url(), &pair.home).await;
    assert_eq!(home.status(), 200, "{}: {}", pair.label, pair.home);
}

/// The one not-found, as alice reads it at a path that names nothing.
async fn the_one_404(world: &DenWorld) -> Answer {
    let response = get(&world.alice, world.base_url(), "/den/nope").await;
    Answer::read(response).await
}

/// Asserts both causes of `pair` answer `expected` as `viewer`.
async fn assert_pair_is_the_one_404(
    viewer: &reqwest::Client,
    world: &DenWorld,
    pair: &MissPair,
    expected: &Answer,
) {
    for path in [&pair.absent, &pair.not_yours] {
        let answer = Answer::read(get(viewer, world.base_url(), path).await).await;

        assert_eq!(&answer, expected, "{}: {path}", pair.label);
    }
}

#[tokio::test]
async fn alice_s_root_is_hers_and_holds_her_three_open_folders_and_posts() {
    let world = pg_app::den_world(api::app).await;
    let expected_node = json!({
        "name": "Alice",
        "type": "user",
        "kind": "directory",
        "open": { "ownLevel": "private" },
    });
    let expected_posts = json!({
        "segments": ["posts"],
        "name": "posts",
        "type": "posts",
        "kind": "directory",
        "mount": true,
        "open": { "ownLevel": "public", "contentNotShown": true },
    });

    let root = ok_json(get(&world.alice, world.base_url(), "/den").await).await;

    assert_eq!(root["node"], expected_node);
    let listing = &root["listing"];
    assert_eq!(
        entry_names(listing),
        ["accounts", "characters", "commissions", "posts"]
    );
    assert!(listing.get("nextPageToken").is_none(), "{listing}");
    let entries = listing["entries"].as_array().expect("entries");
    let (folders, posts) = entries.split_at(3);
    for entry in folders {
        let name = entry["name"].clone();
        assert_eq!(entry["segments"], json!([name]), "{entry}");
        assert_eq!(entry["kind"], "directory", "{entry}");
        assert_eq!(entry["open"], json!({ "ownLevel": "private" }), "{entry}");
        assert!(entry.get("mount").is_none(), "{entry}");
    }
    assert_eq!(posts, [expected_posts]);
}

#[tokio::test]
async fn characters_lists_nothing_and_is_not_flagged() {
    let world = pg_app::den_world(api::app).await;
    let expected_node = json!({
        "segments": ["characters"],
        "name": "characters",
        "type": "user.characters",
        "kind": "directory",
        "open": { "ownLevel": "private" },
    });

    let characters = ok_json(get(&world.alice, world.base_url(), "/den/characters").await).await;

    assert_eq!(characters["node"], expected_node);
    assert_eq!(characters["listing"], json!({}));
}

#[tokio::test]
async fn alice_s_accounts_mounts_supreme_arts() {
    let world = pg_app::den_world(api::app).await;
    let supreme_arts_did = world.supreme_arts.to_string();
    let expected_crumbs = json!([{ "name": "Alice", "type": "user" }]);
    let expected_thing = json!(supreme_arts_did);

    let accounts = ok_json(get(&world.alice, world.base_url(), "/den/accounts").await).await;

    assert_eq!(accounts["node"]["segments"], json!(["accounts"]));
    assert_eq!(accounts["crumbs"], expected_crumbs);
    let listing = &accounts["listing"];
    assert_eq!(entry_names(listing), ["Supreme Arts"]);
    let supreme_arts = &listing["entries"][0];
    assert_eq!(supreme_arts["mount"], true);
    assert_eq!(supreme_arts["type"], "account");
    assert_eq!(
        supreme_arts["segments"],
        json!(["accounts", supreme_arts_did])
    );
    let open = &supreme_arts["open"];
    assert_eq!(open["accountDid"], expected_thing, "{supreme_arts}");
    assert!(open.get("commissionId").is_none(), "{supreme_arts}");
}

#[tokio::test]
async fn untitled_opens_to_its_entries_and_its_attachments_list_the_file() {
    let world = pg_app::den_world(api::app).await;
    let untitled = format!("/den/commissions/{}", world.untitled.id);
    let expected_thing = json!(world.untitled.id.to_string());

    let commission = ok_json(get(&world.alice, world.base_url(), &untitled).await).await;
    let attachments_path = format!("{untitled}/attachments");
    let attachments = ok_json(get(&world.alice, world.base_url(), &attachments_path).await).await;

    assert_eq!(commission["node"]["name"], "Untitled");
    assert_eq!(commission["node"]["type"], "commission");
    assert_eq!(commission["node"]["open"]["commissionId"], expected_thing);
    assert_eq!(
        entry_names(&commission["listing"]),
        ["attachments", "changelog", "products", "slots"]
    );
    assert_eq!(entry_names(&attachments["listing"]), ["ref-abco.png"]);
}

#[tokio::test]
async fn old_sketch_is_listed_only_when_deleted_ones_are_asked_for_and_marked_archived() {
    let world = pg_app::den_world(api::app).await;

    let live = ok_json(get(&world.alice, world.base_url(), "/den/commissions").await).await;
    let everything = ok_json(
        get(
            &world.alice,
            world.base_url(),
            "/den/commissions?includeDeleted=true",
        )
        .await,
    )
    .await;

    assert_eq!(entry_names(&live["listing"]), ["Untitled"]);
    assert_eq!(
        entry_names(&everything["listing"]),
        ["Old sketch", "Untitled"]
    );
    let old_sketch = &everything["listing"]["entries"][0];
    let untitled = &everything["listing"]["entries"][1];
    assert_eq!(old_sketch["open"]["softDelete"], "archived", "{old_sketch}");
    assert!(untitled["open"].get("softDelete").is_none(), "{untitled}");
}

#[tokio::test]
async fn old_sketch_s_typed_path_opens_marked_archived_without_asking_for_deleted_ones() {
    let world = pg_app::den_world(api::app).await;
    let path = format!("/den/commissions/{}", world.old_sketch.id);

    let old_sketch = ok_json(get(&world.alice, world.base_url(), &path).await).await;

    assert_eq!(old_sketch["node"]["name"], "Old sketch");
    assert_eq!(old_sketch["node"]["type"], "commission");
    assert_eq!(old_sketch["node"]["open"]["softDelete"], "archived");
}

#[tokio::test]
async fn each_piece_is_decoded_once_after_the_split() {
    let world = pg_app::den_world(api::app).await;
    let encoded_did = world.supreme_arts.to_string().replace(':', "%3A");

    let encoded_folder =
        ok_json(get(&world.alice, world.base_url(), "/den/%61ccounts").await).await;
    let encoded_account_path = format!("/den/accounts/{encoded_did}");
    let encoded_account =
        ok_json(get(&world.alice, world.base_url(), &encoded_account_path).await).await;

    assert_eq!(encoded_folder["node"]["segments"], json!(["accounts"]));
    assert_eq!(encoded_account["node"]["name"], "Supreme Arts");
}

#[tokio::test]
async fn every_miss_is_the_same_404_byte_for_byte() {
    let world = pg_app::den_world(api::app).await;
    let bob_did = adapter_mem::fixture::bob().to_string();
    let supreme_arts = world.supreme_arts.to_string();
    let night_shift = world.night_shift.to_string();
    let untitled = world.untitled.id;
    let file = world.untitled.file;
    let causes = [
        // A nonexistent child.
        "/den/accounts/did:plc:nope".to_owned(),
        format!("/den/commissions/{}", absent_commission()),
        "/den/characters/did:plc:nope".to_owned(),
        // Malformed pieces: a space, a non-ASCII letter, bytes that aren't UTF-8, a broken
        // escape, an empty piece.
        "/den/accounts/a%20b".to_owned(),
        "/den/accounts/%C3%A9".to_owned(),
        "/den/accounts/%FF".to_owned(),
        "/den/accounts/%2".to_owned(),
        "/den/accounts/".to_owned(),
        "/den//accounts".to_owned(),
        // Seventeen pieces.
        format!("/den/{}accounts", "accounts/".repeat(16)),
        // The empty path.
        "/den/".to_owned(),
        // A path that continues past a file.
        format!("/den/commissions/{untitled}/changelog/more"),
        format!("/den/commissions/{untitled}/attachments/{file}/more"),
        // bob's DID as a piece, under alice's session.
        format!("/den/{bob_did}"),
        format!("/den/accounts/{bob_did}"),
        format!("/den/{bob_did}/accounts"),
        format!("/den/commissions/{bob_did}"),
        // bob's Account, which alice isn't part of.
        format!("/den/accounts/{night_shift}"),
        // An encoded slash stays inside its piece: split first, then decode.
        format!("/den/accounts%2F{supreme_arts}"),
        format!("/den/commissions%2F{untitled}"),
        // Decoded once only: `%252F` is the valid, absent piece `accounts%2F<did>`.
        format!("/den/accounts%252F{supreme_arts}"),
        format!("/den/commissions%252F{untitled}"),
        "/den/%2561ccounts".to_owned(),
    ];
    let first = the_one_404(&world).await;
    let expected_body = json!({
        "type": "urn:zurfur:error:node-not-found",
        "code": "node_not_found",
        "title": "Node not found",
        "detail": "No such node.",
        "status": 404,
    });

    let body: serde_json::Value = serde_json::from_slice(&first.body).expect("a JSON body");
    assert_eq!(first.status, 404);
    assert_eq!(body, expected_body);
    let cache_header = (
        "cache-control".to_owned(),
        PRIVATE_NO_STORE.as_bytes().to_vec(),
    );
    assert!(first.headers.contains(&cache_header), "{first:?}");
    for cause in causes {
        let answer = Answer::read(get(&world.alice, world.base_url(), &cause).await).await;

        assert_eq!(answer, first, "{cause}");
    }
}

#[tokio::test]
async fn a_stranger_gets_the_one_404_for_an_absent_commission_and_one_not_theirs() {
    let world = pg_app::den_world(api::app).await;
    let expected = the_one_404(&world).await;

    for (target, visibility) in den_stranger_targets(&world) {
        world.set_visibility(target.id, visibility).await;
        let commission_path = format!("/den/commissions/{}", target.id);
        let commission = ok_json(get(&world.alice, world.base_url(), &commission_path).await).await;
        assert_eq!(
            commission["node"]["open"]["ownLevel"],
            visibility.to_string(),
            "the pairs run at the level just set"
        );
        for pair in den_stranger_pairs(&target) {
            assert_alice_opens_its_home(&world, &pair).await;

            assert_pair_is_the_one_404(&world.stranger, &world, &pair, &expected).await;
        }
    }
}

#[tokio::test]
async fn a_participant_gets_the_one_404_for_an_absent_key_and_another_commission_s() {
    let world = pg_app::den_world(api::app).await;
    let expected = the_one_404(&world).await;
    let untitled = world.untitled.id;
    let night_shift = world.night_shift.to_string();
    // The folders are bob's to open: each holds a real node of his at the same depth.
    let his_own = [
        format!(
            "/den/commissions/{untitled}/attachments/{}",
            world.untitled.file
        ),
        format!("/den/commissions/{untitled}/slots/{}", world.untitled.slot),
        format!("/den/accounts/{night_shift}/workflows/{}", world.backlog),
    ];

    for path in his_own {
        let opened = get(&world.bob, world.base_url(), &path).await;
        assert_eq!(opened.status(), 200, "{path}");
    }
    for pair in den_participant_pairs(&world) {
        assert_alice_opens_its_home(&world, &pair).await;

        assert_pair_is_the_one_404(&world.bob, &world, &pair, &expected).await;
    }
}

#[tokio::test]
async fn a_pair_s_two_causes_make_the_same_reads_and_never_read_the_row_by_key() {
    let log = CallLog::default();
    let recorder = log.clone();
    let world = pg_app::den_world(move |mut runtime| {
        let commissions =
            RecordingCommissionStore::new(Arc::clone(&runtime.commissions), recorder.clone());
        let workflows = RecordingWorkflowStore::new(Arc::clone(&runtime.workflows), recorder);
        runtime.commissions = Arc::new(commissions);
        runtime.workflows = Arc::new(workflows);
        api::app(runtime)
    })
    .await;

    for (target, visibility) in den_stranger_targets(&world) {
        world.set_visibility(target.id, visibility).await;
        for pair in den_stranger_pairs(&target) {
            let reads = reads_of_each_cause(&log, &world.stranger, &world, &pair).await;

            let label = format!("{visibility}, {}", pair.label);
            assert_eq!(reads.absent, reads.not_yours, "{label}");
            assert!(!reads.absent.is_empty(), "{label}: no store read recorded");
            assert!(
                !reads.absent.contains(&"find"),
                "{label}: {:?}",
                reads.absent
            );
        }
    }
    for pair in den_participant_pairs(&world) {
        let reads = reads_of_each_cause(&log, &world.bob, &world, &pair).await;

        let label = pair.label;
        assert_eq!(reads.absent, reads.not_yours, "{label}");
        assert!(!reads.absent.is_empty(), "{label}: no store read recorded");
        assert!(
            !reads.absent.contains(&"find_workflow"),
            "{label}: {:?}",
            reads.absent
        );
    }
}

/// The commission- and Workflow-store reads each cause of a pair made, in order.
struct CauseReads {
    absent: Vec<&'static str>,
    not_yours: Vec<&'static str>,
}

/// The store reads each cause of `pair` makes as `viewer`, each answered `404`.
async fn reads_of_each_cause(
    log: &CallLog,
    viewer: &reqwest::Client,
    world: &DenWorld,
    pair: &MissPair,
) -> CauseReads {
    let absent = reads_of(log, viewer, world, pair.label, &pair.absent).await;
    let not_yours = reads_of(log, viewer, world, pair.label, &pair.not_yours).await;
    CauseReads { absent, not_yours }
}

/// The store reads `GET {path}` makes as `viewer`, after checking it is a `404`.
async fn reads_of(
    log: &CallLog,
    viewer: &reqwest::Client,
    world: &DenWorld,
    label: &str,
    path: &str,
) -> Vec<&'static str> {
    let before = log.events().len();
    let response = get(viewer, world.base_url(), path).await;
    assert_eq!(response.status(), 404, "{label}: {path}");
    log.events().split_off(before)
}

#[tokio::test]
async fn a_row_whose_visibility_no_parser_reads_is_the_same_404() {
    let world = pg_app::den_world(api::app).await;
    let expected = the_one_404(&world).await;

    world.store_unparsable_visibility(world.old_sketch.id).await;

    for pair in den_stranger_pairs(&world.old_sketch) {
        assert_pair_is_the_one_404(&world.stranger, &world, &pair, &expected).await;
    }
    for pair in den_participant_pairs(&world) {
        assert_pair_is_the_one_404(&world.bob, &world, &pair, &expected).await;
    }
}

#[tokio::test]
async fn with_old_sketch_s_visibility_unreadable_alice_s_commissions_list_it_at_private() {
    let world = pg_app::den_world(api::app).await;
    world.store_unparsable_visibility(world.old_sketch.id).await;

    let everything = ok_json(
        get(
            &world.alice,
            world.base_url(),
            "/den/commissions?includeDeleted=true",
        )
        .await,
    )
    .await;

    assert_eq!(
        entry_names(&everything["listing"]),
        ["Old sketch", "Untitled"]
    );
    let old_sketch = &everything["listing"]["entries"][0];
    assert_eq!(old_sketch["open"]["ownLevel"], "private", "{old_sketch}");
}

#[tokio::test]
async fn with_old_sketch_s_visibility_unreadable_its_typed_path_opens_archived_at_private() {
    let world = pg_app::den_world(api::app).await;
    world.store_unparsable_visibility(world.old_sketch.id).await;
    let path = format!("/den/commissions/{}", world.old_sketch.id);

    let old_sketch = ok_json(get(&world.alice, world.base_url(), &path).await).await;

    assert_eq!(old_sketch["node"]["name"], "Old sketch");
    assert_eq!(old_sketch["node"]["open"]["softDelete"], "archived");
    assert_eq!(old_sketch["node"]["open"]["ownLevel"], "private");
}

#[tokio::test]
async fn with_old_sketch_s_lifecycle_unreadable_its_typed_path_still_opens() {
    let world = pg_app::den_world(api::app).await;
    world.store_unparsable_lifecycle(world.old_sketch.id).await;
    let path = format!("/den/commissions/{}", world.old_sketch.id);

    let old_sketch = ok_json(get(&world.alice, world.base_url(), &path).await).await;

    assert_eq!(old_sketch["node"]["name"], "Old sketch");
    assert_eq!(old_sketch["node"]["open"]["softDelete"], "archived");
}

#[tokio::test]
async fn a_title_stored_before_the_rule_lists_and_opens_as_unnamed_for_each_participant() {
    let world = pg_app::den_world(api::app).await;
    world.store_legacy_title(world.untitled.id).await;
    let typed_path = format!("/den/commissions/{}", world.untitled.id);

    for (viewer, label) in [(&world.alice, "alice"), (&world.bob, "bob")] {
        let commissions = ok_json(get(viewer, world.base_url(), "/den/commissions").await).await;
        let opened = ok_json(get(viewer, world.base_url(), &typed_path).await).await;

        assert_eq!(entry_names(&commissions["listing"]), [UNNAMED], "{label}");
        assert_eq!(opened["node"]["name"], UNNAMED, "{label}");
        assert_eq!(opened["node"]["type"], "commission", "{label}");
    }
}

#[tokio::test]
async fn a_title_stored_before_the_rule_is_still_the_one_404_for_a_stranger() {
    let world = pg_app::den_world(api::app).await;
    let expected = the_one_404(&world).await;
    world.store_legacy_title(world.untitled.id).await;

    for pair in den_stranger_pairs(&world.untitled) {
        assert_alice_opens_its_home(&world, &pair).await;

        assert_pair_is_the_one_404(&world.stranger, &world, &pair, &expected).await;
    }
}

#[tokio::test]
async fn an_archived_title_stored_before_the_rule_lists_as_unnamed_and_marked_archived() {
    let world = pg_app::den_world(api::app).await;
    world.store_legacy_title(world.old_sketch.id).await;
    let old_sketch_segments = json!(["commissions", world.old_sketch.id.to_string()]);
    let typed_path = format!("/den/commissions/{}", world.old_sketch.id);

    let everything = ok_json(
        get(
            &world.alice,
            world.base_url(),
            "/den/commissions?includeDeleted=true",
        )
        .await,
    )
    .await;
    let opened = ok_json(get(&world.alice, world.base_url(), &typed_path).await).await;

    let entries = everything["listing"]["entries"]
        .as_array()
        .expect("entries");
    let old_sketch = entries
        .iter()
        .find(|entry| entry["segments"] == old_sketch_segments)
        .unwrap_or_else(|| panic!("Old sketch is listed: {everything}"));
    assert_eq!(old_sketch["name"], UNNAMED, "{old_sketch}");
    assert_eq!(old_sketch["open"]["softDelete"], "archived", "{old_sketch}");
    assert_eq!(opened["node"]["name"], UNNAMED);
    assert_eq!(opened["node"]["open"]["softDelete"], "archived");
}

/// What the fill script printed, and whether it succeeded.
struct FillRun {
    success: bool,
    stdout: String,
    stderr: String,
}

/// Runs `scripts/den-fill.sh` against `served` with `cookie` written to its stdin, as a user
/// pastes it at the hidden prompt.
async fn run_den_fill(served: &pg_app::PgServed, cookie: &str) -> FillRun {
    let mut child = Command::new("bash")
        .arg(DEN_FILL)
        .env("ZURFUR_API_UPSTREAM", &served.base_url)
        .env("ZURFUR_PUBLIC_URL", &served.base_url)
        .env("ZURFUR_FILL_PSQL", served.psql_command())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("bash runs the fill script");
    let mut stdin = child.stdin.take().expect("the script's stdin");
    let pasted = format!("{cookie}\n");
    stdin
        .write_all(pasted.as_bytes())
        .await
        .expect("the cookie is written to the script's stdin");
    drop(stdin);
    let output = child
        .wait_with_output()
        .await
        .expect("the fill script finishes");
    FillRun {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The path of the listing entry named `name` in `body`.
fn entry_path(body: &Value, name: &str) -> String {
    let entries = body["listing"]["entries"].as_array().expect("entries");
    let entry = entries
        .iter()
        .find(|entry| entry["name"] == name)
        .unwrap_or_else(|| panic!("{name} is listed: {body}"));
    let segments = entry["segments"].as_array().expect("segments");
    let pieces: Vec<&str> = segments
        .iter()
        .map(|segment| segment.as_str().expect("a text segment"))
        .collect();
    format!("/den/{}", pieces.join("/"))
}

/// `entry_names` of the listing at `path`, sorted, as `signed_in` reads it.
async fn sorted_names(signed_in: &SignedIn, served: &pg_app::PgServed, path: &str) -> Vec<String> {
    let body = ok_json(get(&signed_in.client, &served.base_url, path).await).await;
    let mut names = entry_names(&body["listing"]);
    names.sort();
    names
}

#[tokio::test]
async fn the_fill_script_fills_the_signed_in_den_with_a_live_and_an_archived_commission() {
    let served = pg_app::serve(api::app).await;
    let filler = Profile::new(Did::from(FILLER_DID.to_owned()), "filler.test");
    let signed_in = served.sign_in_with_cookie(filler).await;

    let run = run_den_fill(&served, &signed_in.session_cookie).await;

    assert!(run.success, "the fill script fails: {}", run.stderr);
    assert!(
        !run.stdout.contains(&signed_in.session_cookie),
        "stdout holds the cookie"
    );
    assert!(
        !run.stderr.contains(&signed_in.session_cookie),
        "stderr holds the cookie"
    );
    assert!(run.stdout.contains("Sign out"), "{}", run.stdout);

    let live = ok_json(get(&signed_in.client, &served.base_url, "/den/commissions").await).await;
    let everything = ok_json(
        get(
            &signed_in.client,
            &served.base_url,
            "/den/commissions?includeDeleted=true",
        )
        .await,
    )
    .await;
    assert_eq!(entry_names(&live["listing"]), ["Sketch request"]);
    assert_eq!(
        entry_names(&everything["listing"]),
        ["Old badge", "Sketch request"]
    );
    let old_badge = &everything["listing"]["entries"][0];
    let sketch_request = &everything["listing"]["entries"][1];
    assert_eq!(old_badge["open"]["softDelete"], "archived", "{old_badge}");
    assert!(
        sketch_request["open"].get("softDelete").is_none(),
        "{sketch_request}"
    );

    let filled = [
        (
            "Sketch request",
            ["pose.png", "reference.png"],
            vec!["Main character", "Second character"],
        ),
        (
            "Old badge",
            ["badge-draft.png", "badge-final.png"],
            vec!["Badge wearer"],
        ),
    ];
    for (title, files, slots) in filled {
        let commission = entry_path(&everything, title);
        let attachments_path = format!("{commission}/attachments");
        let slots_path = format!("{commission}/slots");

        let entries = sorted_names(&signed_in, &served, &commission).await;
        let attachments = sorted_names(&signed_in, &served, &attachments_path).await;
        let declared = sorted_names(&signed_in, &served, &slots_path).await;

        assert_eq!(
            entries,
            ["attachments", "changelog", "products", "slots"],
            "{title}"
        );
        assert_eq!(attachments, files, "{title}");
        assert_eq!(declared, slots, "{title}");
    }
}
