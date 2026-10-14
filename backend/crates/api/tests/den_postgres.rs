//! `GET /den` and `GET /den/{*path}` on the PostgreSQL stores, over the den world seeded
//! through the app's routes: alice's root, her membership mount, the archived "Old sketch",
//! and the one 404, byte for byte, for every cause, the stranger's and the Participant's
//! pairs of causes included. Requires a container runtime socket.

use std::sync::Arc;

use domain::elements::commission::Visibility;
use serde_json::json;
use test_support::{
    den::{Answer, PRIVATE_NO_STORE, entry_names, get, ok_json},
    pg_app::{
        self, DenWorld, SeededCommission, absent_commission, absent_file, absent_slot,
        absent_workflow,
    },
    recording::{CallLog, RecordingCommissionStore, RecordingWorkflowStore},
};

/// Two causes of one miss at the same depth: a path to nothing, and a path to a real node
/// that isn't the viewer's to open. Each must answer the one 404.
struct MissPair {
    /// What the pair is, for assertion messages.
    label: &'static str,
    /// The path to nothing.
    absent: String,
    /// The path to a real node the viewer may not open.
    not_yours: String,
    /// The same real node at its home in alice's Den, where she opens it.
    home: String,
}

/// The stranger's pairs against `target`, a commission they take part in nothing of: the
/// commission itself, its `changelog`, and its file.
fn stranger_pairs(target: &SeededCommission) -> [MissPair; 3] {
    let absent = absent_commission();
    let commission = target.id;
    let file = target.file;
    let absent_file = absent_file();
    let commission_path = format!("/den/commissions/{commission}");
    let changelog_path = format!("/den/commissions/{commission}/changelog");
    let file_path = format!("/den/commissions/{commission}/attachments/{file}");
    let commission_pair = MissPair {
        label: "the commission",
        absent: format!("/den/commissions/{absent}"),
        not_yours: commission_path.clone(),
        home: commission_path,
    };
    let changelog_pair = MissPair {
        label: "its changelog",
        absent: format!("/den/commissions/{absent}/changelog"),
        not_yours: changelog_path.clone(),
        home: changelog_path,
    };
    let file_pair = MissPair {
        label: "its file",
        absent: format!("/den/commissions/{absent}/attachments/{absent_file}"),
        not_yours: file_path.clone(),
        home: file_path,
    };
    [commission_pair, changelog_pair, file_pair]
}

/// The commissions the stranger's pairs run against, each at the visibility the pair needs:
/// "Untitled" Private and live, then the archived "Old sketch" Private, Public and Listed.
#[expect(
    clippy::let_and_return,
    reason = "the style rulebook names a multi-line construction before returning it"
)]
fn stranger_targets(world: &DenWorld) -> [(SeededCommission, Visibility); 4] {
    let targets = [
        (world.untitled, Visibility::Private),
        (world.old_sketch, Visibility::Private),
        (world.old_sketch, Visibility::Public),
        (world.old_sketch, Visibility::Listed),
    ];
    targets
}

/// bob's pairs as a Participant of "Untitled": a missing file key against "Old sketch"'s
/// real one under "Untitled", a missing slot against "Old sketch"'s, and in his own
/// Account's `workflows`, a missing Workflow against alice's Account's.
fn participant_pairs(world: &DenWorld) -> [MissPair; 3] {
    let untitled = world.untitled.id;
    let old_sketch = world.old_sketch.id;
    let night_shift = world.night_shift.to_string();
    let supreme_arts = world.supreme_arts.to_string();
    let foreign_file = world.old_sketch.file;
    let foreign_slot = world.old_sketch.slot;
    let foreign_workflow = world.queue;
    let absent_file = absent_file();
    let absent_slot = absent_slot();
    let absent_workflow = absent_workflow();
    let file_pair = MissPair {
        label: "a file key",
        absent: format!("/den/commissions/{untitled}/attachments/{absent_file}"),
        not_yours: format!("/den/commissions/{untitled}/attachments/{foreign_file}"),
        home: format!("/den/commissions/{old_sketch}/attachments/{foreign_file}"),
    };
    let slot_pair = MissPair {
        label: "a slot",
        absent: format!("/den/commissions/{untitled}/slots/{absent_slot}"),
        not_yours: format!("/den/commissions/{untitled}/slots/{foreign_slot}"),
        home: format!("/den/commissions/{old_sketch}/slots/{foreign_slot}"),
    };
    let workflow_pair = MissPair {
        label: "a Workflow",
        absent: format!("/den/accounts/{night_shift}/workflows/{absent_workflow}"),
        not_yours: format!("/den/accounts/{night_shift}/workflows/{foreign_workflow}"),
        home: format!("/den/accounts/{supreme_arts}/workflows/{foreign_workflow}"),
    };
    [file_pair, slot_pair, workflow_pair]
}

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
        "type": "post",
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
    let expected_crumbs = json!([{ "name": "Alice" }]);

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
    assert!(supreme_arts.get("open").is_some(), "{supreme_arts}");
}

#[tokio::test]
async fn untitled_opens_to_its_entries_and_its_attachments_list_the_file() {
    let world = pg_app::den_world(api::app).await;
    let untitled = format!("/den/commissions/{}", world.untitled.id);

    let commission = ok_json(get(&world.alice, world.base_url(), &untitled).await).await;
    let attachments_path = format!("{untitled}/attachments");
    let attachments = ok_json(get(&world.alice, world.base_url(), &attachments_path).await).await;

    assert_eq!(commission["node"]["name"], "Untitled");
    assert_eq!(commission["node"]["type"], "commission");
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

    for (target, visibility) in stranger_targets(&world) {
        world.set_visibility(target.id, visibility.clone()).await;
        let commission_path = format!("/den/commissions/{}", target.id);
        let commission = ok_json(get(&world.alice, world.base_url(), &commission_path).await).await;
        assert_eq!(
            commission["node"]["open"]["ownLevel"],
            visibility.to_string(),
            "the pairs run at the level just set"
        );
        for pair in stranger_pairs(&target) {
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
    for pair in participant_pairs(&world) {
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

    for (target, visibility) in stranger_targets(&world) {
        world.set_visibility(target.id, visibility.clone()).await;
        for pair in stranger_pairs(&target) {
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
    for pair in participant_pairs(&world) {
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

    for pair in stranger_pairs(&world.old_sketch) {
        assert_pair_is_the_one_404(&world.stranger, &world, &pair, &expected).await;
    }
    for pair in participant_pairs(&world) {
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
