//! `GET /commissions/{id}/changelog` and `GET /commissions/{id}/files/{key}` behind den's
//! check, on the PostgreSQL stores over the den world: each pair of not-found causes answers
//! byte for byte the same and makes the same store reads, and a stranger's causes never read
//! by key. Requires a container runtime socket.

use std::sync::Arc;

use domain::elements::commission::Visibility;
use test_support::{
    den::{Answer, get},
    pg_app::{
        self, DenWorld, MissPair, commission_read_stranger_pairs, download_participant_pair,
        public_live_download_pair, stranger_targets,
    },
    recording::{
        CallLog, NEVER_FOR_A_MISSING_FILE, NEVER_FOR_A_STRANGER, RecordingChangelogStore,
        RecordingCommissionStore, RecordingFileStore, assert_reads_none_of,
    },
};

/// Asserts alice opens `pair`'s real node at its home, so the pair never compares two
/// absent nodes.
async fn assert_alice_opens_its_home(world: &DenWorld, pair: &MissPair) {
    let home = get(&world.alice, world.base_url(), &pair.home).await;
    assert_eq!(home.status(), 200, "{}: {}", pair.label, pair.home);
}

/// Both causes of `pair` read as `viewer`, every client-visible byte of each.
async fn answers_of(viewer: &reqwest::Client, world: &DenWorld, pair: &MissPair) -> [Answer; 2] {
    let absent = Answer::read(get(viewer, world.base_url(), &pair.absent).await).await;
    let not_yours = Answer::read(get(viewer, world.base_url(), &pair.not_yours).await).await;
    [absent, not_yours]
}

/// Asserts `answer` is a `404` whose problem code is `code`.
fn assert_not_found_with(answer: &Answer, code: &str, label: &str) {
    let body: serde_json::Value = serde_json::from_slice(&answer.body).expect("a JSON body");
    assert_eq!(answer.status, 404, "{label}: {body}");
    assert_eq!(body["code"], code, "{label}: {body}");
}

/// The problem code both of a stranger's causes answer with, on either read.
const STRANGER_CODE: &str = "commission_not_found";

#[tokio::test]
async fn a_stranger_s_absent_commission_and_one_not_theirs_answer_alike_through_both_reads() {
    let world = pg_app::den_world(api::app).await;

    for (target, visibility) in stranger_targets(&world) {
        world.set_visibility(target.id, visibility.clone()).await;
        for pair in commission_read_stranger_pairs(&target) {
            assert_alice_opens_its_home(&world, &pair).await;

            let [absent, not_yours] = answers_of(&world.stranger, &world, &pair).await;

            let label = format!("{visibility}, {}", pair.label);
            assert_not_found_with(&absent, STRANGER_CODE, &label);
            assert_eq!(not_yours, absent, "{label}");
        }
    }
}

#[tokio::test]
async fn bob_s_absent_key_and_another_commission_s_answer_alike_through_the_download() {
    let world = pg_app::den_world(api::app).await;
    let pair = download_participant_pair(&world);
    let his_own = format!(
        "/commissions/{}/files/{}",
        world.untitled.id, world.untitled.file
    );
    let opened = get(&world.bob, world.base_url(), &his_own).await;
    assert_eq!(opened.status(), 200, "bob downloads Untitled's file");
    assert_alice_opens_its_home(&world, &pair).await;

    let [absent, not_yours] = answers_of(&world.bob, &world, &pair).await;

    assert_not_found_with(&absent, "file_not_found", pair.label);
    assert_eq!(not_yours, absent);
}

#[tokio::test]
async fn bob_reads_untitled_s_changelog() {
    let world = pg_app::den_world(api::app).await;
    let path = format!("/commissions/{}/changelog", world.untitled.id);

    let read = get(&world.bob, world.base_url(), &path).await;

    assert_eq!(read.status(), 200);
}

/// The live levels a stranger can reach a commission's node at, or a card of it, and each
/// read's pair there with the problem code both of its causes answer: the changelog's
/// `changelog` stays closed at both; the download opens the node at Public, so its pair is an
/// absent key against the real one under it, and refuses it at Listed.
fn live_pairs(world: &DenWorld) -> [(Visibility, MissPair, &'static str); 4] {
    let [changelog, download] = commission_read_stranger_pairs(&world.untitled);
    let public_download = public_live_download_pair(&world.untitled);
    [
        (Visibility::Public, changelog.clone(), STRANGER_CODE),
        (Visibility::Public, public_download, "file_not_found"),
        (Visibility::Listed, changelog, STRANGER_CODE),
        (Visibility::Listed, download, STRANGER_CODE),
    ]
}

#[tokio::test]
async fn on_a_live_public_or_listed_commission_a_stranger_s_causes_answer_alike() {
    let world = pg_app::den_world(api::app).await;

    for (visibility, pair, code) in live_pairs(&world) {
        world
            .set_visibility(world.untitled.id, visibility.clone())
            .await;
        assert_alice_opens_its_home(&world, &pair).await;

        let [absent, not_yours] = answers_of(&world.stranger, &world, &pair).await;

        let label = format!("{visibility}, live, {}", pair.label);
        assert_not_found_with(&absent, code, &label);
        assert_eq!(not_yours, absent, "{label}");
    }
}

#[tokio::test]
async fn with_old_sketch_s_visibility_unreadable_a_stranger_s_causes_still_answer_alike() {
    let world = pg_app::den_world(api::app).await;
    world.store_unparsable_visibility(world.old_sketch.id).await;

    for pair in commission_read_stranger_pairs(&world.old_sketch) {
        let [absent, not_yours] = answers_of(&world.stranger, &world, &pair).await;

        assert_not_found_with(&absent, STRANGER_CODE, pair.label);
        assert_eq!(not_yours, absent, "{}", pair.label);
    }
    let pair = download_participant_pair(&world);
    let [absent, not_yours] = answers_of(&world.bob, &world, &pair).await;
    assert_not_found_with(&absent, "file_not_found", pair.label);
    assert_eq!(not_yours, absent, "{}", pair.label);
}

#[tokio::test]
async fn with_the_live_untitled_s_visibility_unreadable_a_stranger_s_causes_answer_alike() {
    let world = pg_app::den_world(api::app).await;
    world.store_unparsable_visibility(world.untitled.id).await;
    let his_own = format!(
        "/commissions/{}/files/{}",
        world.untitled.id, world.untitled.file
    );

    for pair in commission_read_stranger_pairs(&world.untitled) {
        let [absent, not_yours] = answers_of(&world.stranger, &world, &pair).await;

        assert_not_found_with(&absent, STRANGER_CODE, pair.label);
        assert_eq!(not_yours, absent, "{}", pair.label);
    }
    let download = get(&world.bob, world.base_url(), &his_own).await;
    assert_eq!(
        download.status(),
        200,
        "bob still downloads Untitled's file"
    );
}

#[tokio::test]
async fn with_old_sketch_s_visibility_unreadable_alice_still_downloads_and_reads_it() {
    let world = pg_app::den_world(api::app).await;
    world.store_unparsable_visibility(world.old_sketch.id).await;
    let path = format!(
        "/commissions/{}/files/{}",
        world.old_sketch.id, world.old_sketch.file
    );

    let download = get(&world.alice, world.base_url(), &path).await;

    assert_eq!(download.status(), 200);
    let bytes = download.bytes().await.expect("the file's bytes");
    assert_eq!(bytes.as_ref(), b"den world image bytes");
    let changelog_path = format!("/commissions/{}/changelog", world.old_sketch.id);
    let changelog = get(&world.alice, world.base_url(), &changelog_path).await;
    assert_eq!(
        changelog.status(),
        200,
        "alice reads Old sketch's changelog"
    );
}

/// The store reads each cause of a pair made, in order.
struct CauseReads {
    absent: Vec<&'static str>,
    not_yours: Vec<&'static str>,
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

/// The store reads each cause of `pair` makes as `viewer`, each answered `404`.
async fn reads_of_each_cause(
    log: &CallLog,
    viewer: &reqwest::Client,
    world: &DenWorld,
    label: &str,
    pair: &MissPair,
) -> CauseReads {
    let absent = reads_of(log, viewer, world, label, &pair.absent).await;
    let not_yours = reads_of(log, viewer, world, label, &pair.not_yours).await;
    CauseReads { absent, not_yours }
}

#[tokio::test]
async fn a_pair_s_two_causes_make_the_same_reads_and_a_stranger_s_never_read_by_key() {
    let log = CallLog::default();
    let recorder = log.clone();
    let world = pg_app::den_world(move |mut runtime| {
        let commissions =
            RecordingCommissionStore::new(Arc::clone(&runtime.commissions), recorder.clone());
        let files = RecordingFileStore::new(Arc::clone(&runtime.files), recorder.clone());
        let changelog = RecordingChangelogStore::new(Arc::clone(&runtime.changelog), recorder);
        runtime.commissions = Arc::new(commissions);
        runtime.files = Arc::new(files);
        runtime.changelog = Arc::new(changelog);
        api::app(runtime)
    })
    .await;

    for (target, visibility) in stranger_targets(&world) {
        world.set_visibility(target.id, visibility.clone()).await;
        for pair in commission_read_stranger_pairs(&target) {
            let label = format!("{visibility}, {}", pair.label);
            let reads = reads_of_each_cause(&log, &world.stranger, &world, &label, &pair).await;

            assert_eq!(reads.absent, ["find_summary", "is_participant"], "{label}");
            assert_eq!(reads.not_yours, reads.absent, "{label}");
            assert_reads_none_of(&reads.absent, &NEVER_FOR_A_STRANGER, &label);
        }
    }
    for (visibility, pair, code) in live_pairs(&world) {
        world
            .set_visibility(world.untitled.id, visibility.clone())
            .await;
        let label = format!("{visibility}, live, {}", pair.label);
        let reads = reads_of_each_cause(&log, &world.stranger, &world, &label, &pair).await;

        if code == STRANGER_CODE {
            assert_eq!(reads.absent, ["find_summary", "is_participant"], "{label}");
        }
        assert_eq!(reads.not_yours, reads.absent, "{label}");
        assert_reads_none_of(&reads.absent, &NEVER_FOR_A_STRANGER, &label);
    }
    let pair = download_participant_pair(&world);
    let reads = reads_of_each_cause(&log, &world.bob, &world, pair.label, &pair).await;
    assert!(reads.absent.contains(&"files"), "{:?}", reads.absent);
    assert_eq!(reads.not_yours, reads.absent);
    assert_reads_none_of(&reads.absent, &NEVER_FOR_A_MISSING_FILE, pair.label);
}
