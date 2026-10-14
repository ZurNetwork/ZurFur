//! Each pair of not-found causes at the same depth (absent; present but not the viewer's) runs
//! the same SQL statements in the same order on PostgreSQL, through the den routes and through
//! commission's reads behind den's check, with the profile cache warm and then cold. The
//! statements are read off the wire by the recording proxy; only bound values may differ.
//! Requires a container runtime socket.

use domain::elements::did::Did;
use strum::VariantArray as _;
use test_support::{
    den::get,
    pg_app::{
        self, COMMISSION_NOT_FOUND, DenWorld, MissPair, ProfileCacheState, Statement, StatementLog,
        commission_read_live_pairs, commission_read_stranger_pairs, den_account_pair,
        den_character_pair, den_participant_pairs, den_stranger_pairs, download_participant_pair,
        stranger_targets,
    },
};

/// The whole commission row, read by key: never for a stranger through the den routes.
const WHOLE_COMMISSION: &str = "commission/find";

/// The Workflow read by key: the shortcut the den routes must not take for a Participant.
const WORKFLOW_BY_KEY: &str = "workflow/find";

/// What den's check reads of a commission for a stranger, and all of it: the narrow
/// projection and the viewer's participant row.
const STRANGER_CHECK: [&str; 2] = ["commission/find_summary", "commission/is_participant"];

/// What a stranger's causes must never run on commission's reads: the whole row, anything by
/// key, a file's name or bytes, and the changelog's entries.
const NEVER_FOR_A_STRANGER: [&str; 6] = [
    "commission/find",
    "commission/files",
    "commission/find_file",
    "file/filename",
    "file/get",
    "changelog/entries",
];

/// What a download of a file the viewer's check can't find must never run: the file's row,
/// its name or its bytes, by key.
const NEVER_FOR_A_MISSING_FILE: [&str; 3] = ["commission/find_file", "file/filename", "file/get"];

/// The query directories whose statements read commission data.
const COMMISSION_DATA: [&str; 3] = ["commission/", "file/", "changelog/"];

/// The den world served through the recording proxy, and the log it fills.
struct Recorded {
    world: DenWorld,
    statements: StatementLog,
}

/// The den world on PostgreSQL, every statement it executes logged.
async fn recorded_world() -> Recorded {
    let statements = StatementLog::default();
    let world = pg_app::den_world_recording(api::app, &statements).await;
    Recorded { world, statements }
}

/// One reader of a pair: their signed-in client and their DID.
struct Reader<'a> {
    client: &'a reqwest::Client,
    did: Did,
}

/// The stranger, as a reader.
async fn stranger(world: &DenWorld) -> Reader<'_> {
    reader(&world.stranger, world).await
}

/// bob, as a reader.
fn bob(world: &DenWorld) -> Reader<'_> {
    let did = adapter_mem::fixture::bob().did().clone();
    Reader {
        client: &world.bob,
        did,
    }
}

/// `client` as a reader, their DID read from `GET /me`.
async fn reader<'a>(client: &'a reqwest::Client, world: &DenWorld) -> Reader<'a> {
    let me = get(client, world.base_url(), "/me").await;
    let me: serde_json::Value = me.json().await.expect("a JSON body");
    let did = me["did"].as_str().expect("the reader's DID").to_owned();
    Reader {
        client,
        did: Did::from(did),
    }
}

/// The statements `GET {path}` runs as `reader` from `cache`, after checking it is a `404`.
async fn statements_of(
    recorded: &Recorded,
    reader: &Reader<'_>,
    cache: ProfileCacheState,
    label: &str,
    path: &str,
) -> Vec<Statement> {
    recorded.world.set_profile_cache(cache).await;
    let mark = recorded.statements.mark();
    let response = get(reader.client, recorded.world.base_url(), path).await;
    assert_eq!(response.status(), 404, "{label}, {cache}: {path}");
    recorded.statements.since(mark)
}

/// The statements' query names (or SQL), for reading a failure.
fn names(statements: &[Statement]) -> Vec<String> {
    statements.iter().map(Statement::to_string).collect()
}

/// The statements' SQL text, in order: what the two causes must share.
fn texts(statements: &[Statement]) -> Vec<&str> {
    statements
        .iter()
        .map(|statement| statement.sql.as_str())
        .collect()
}

/// Asserts each cause of `pair` runs the same statements as `reader`, from each cache state,
/// and that every profile statement reads the reader's own DID; returns the statements the
/// absent cause ran, by cache state.
async fn assert_same_statements(
    recorded: &Recorded,
    reader: &Reader<'_>,
    label: &str,
    pair: &MissPair,
) -> Vec<Vec<Statement>> {
    let mut by_state = Vec::new();
    for cache in ProfileCacheState::VARIANTS.iter().copied() {
        let absent = statements_of(recorded, reader, cache, label, &pair.absent).await;
        let not_yours = statements_of(recorded, reader, cache, label, &pair.not_yours).await;

        assert!(
            !absent.is_empty(),
            "{label}, {cache}: no statement recorded"
        );
        assert_eq!(
            texts(&absent),
            texts(&not_yours),
            "{label}, {cache}: {:?} against {:?}",
            names(&absent),
            names(&not_yours)
        );
        assert_profile_reads_are_the_reader_s(&absent, reader, label, cache);
        by_state.push(absent);
    }
    by_state
}

/// Asserts every profile-cache statement in `statements` is keyed by `reader`'s own DID.
fn assert_profile_reads_are_the_reader_s(
    statements: &[Statement],
    reader: &Reader<'_>,
    label: &str,
    cache: ProfileCacheState,
) {
    let did = reader.did.as_ref().as_bytes();
    for statement in statements {
        let reads_a_profile = statement
            .query_name()
            .is_some_and(|name| name.starts_with("profile/"));
        if !reads_a_profile {
            continue;
        }
        let key = statement.parameters.first().cloned().flatten();
        assert_eq!(key.as_deref(), Some(did), "{label}, {cache}: {statement}");
    }
}

/// Asserts `statements` run none of `never`.
fn assert_runs_none_of(statements: &[Statement], never: &[&str], label: &str) {
    let ran = names(statements);
    for name in never {
        assert!(
            !ran.iter().any(|run| run == name),
            "{label}: {name} in {ran:?}"
        );
    }
}

/// The statements among `statements` that read commission data, by name.
fn commission_data(statements: &[Statement]) -> Vec<String> {
    names(statements)
        .into_iter()
        .filter(|name| COMMISSION_DATA.iter().any(|dir| name.starts_with(dir)))
        .collect()
}

#[tokio::test]
async fn a_stranger_s_causes_run_the_same_statements_through_the_den_routes() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let stranger = stranger(world).await;

    for (target, visibility) in stranger_targets(world) {
        world.set_visibility(target.id, visibility.clone()).await;
        let lifecycle = world.archived_or_live(&target);
        for pair in den_stranger_pairs(&target) {
            let label = format!("{visibility} {lifecycle}, {}", pair.label);

            let runs = assert_same_statements(&recorded, &stranger, &label, &pair).await;

            for statements in runs {
                assert_runs_none_of(&statements, &[WHOLE_COMMISSION], &label);
            }
        }
    }
    let account = den_account_pair(world);
    assert_same_statements(&recorded, &stranger, account.label, &account).await;
}

#[tokio::test]
async fn a_stranger_s_unknown_did_and_another_user_s_run_the_same_statements_under_characters() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let stranger = stranger(world).await;
    let characters = den_character_pair();

    assert_same_statements(&recorded, &stranger, characters.label, &characters).await;
}

#[tokio::test]
async fn a_participant_s_causes_run_the_same_statements_through_the_den_routes() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let bob = bob(world);

    for pair in den_participant_pairs(world) {
        let runs = assert_same_statements(&recorded, &bob, pair.label, &pair).await;

        for statements in runs {
            assert_runs_none_of(&statements, &[WORKFLOW_BY_KEY], pair.label);
        }
    }
}

#[tokio::test]
async fn a_stranger_s_causes_run_the_same_statements_through_commission_s_reads() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let stranger = stranger(world).await;

    for (target, visibility) in stranger_targets(world) {
        world.set_visibility(target.id, visibility.clone()).await;
        let lifecycle = world.archived_or_live(&target);
        for pair in commission_read_stranger_pairs(&target) {
            let label = format!("{visibility} {lifecycle}, {}", pair.label);

            let runs = assert_same_statements(&recorded, &stranger, &label, &pair).await;

            for statements in runs {
                assert_eq!(commission_data(&statements), STRANGER_CHECK, "{label}");
            }
        }
    }
    for (visibility, pair, code) in commission_read_live_pairs(world) {
        world
            .set_visibility(world.untitled.id, visibility.clone())
            .await;
        let label = format!("{visibility}, live, {}", pair.label);

        let runs = assert_same_statements(&recorded, &stranger, &label, &pair).await;

        for statements in runs {
            if code == COMMISSION_NOT_FOUND {
                assert_eq!(commission_data(&statements), STRANGER_CHECK, "{label}");
            }
            assert_runs_none_of(&statements, &NEVER_FOR_A_STRANGER, &label);
        }
    }
}

#[tokio::test]
async fn a_participant_s_causes_run_the_same_statements_through_the_download() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let bob = bob(world);
    let pair = download_participant_pair(world);

    let runs = assert_same_statements(&recorded, &bob, pair.label, &pair).await;

    for statements in runs {
        assert_runs_none_of(&statements, &NEVER_FOR_A_MISSING_FILE, pair.label);
    }
}

#[tokio::test]
async fn with_a_visibility_no_parser_reads_each_pair_still_runs_the_same_statements() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let stranger = stranger(world).await;
    let bob = bob(world);
    world.store_unparsable_visibility(world.old_sketch.id).await;
    world.store_unparsable_visibility(world.untitled.id).await;

    for target in [world.old_sketch, world.untitled] {
        for pair in den_stranger_pairs(&target) {
            let runs = assert_same_statements(&recorded, &stranger, pair.label, &pair).await;

            for statements in runs {
                assert_runs_none_of(&statements, &[WHOLE_COMMISSION], pair.label);
            }
        }
        for pair in commission_read_stranger_pairs(&target) {
            let runs = assert_same_statements(&recorded, &stranger, pair.label, &pair).await;

            for statements in runs {
                assert_eq!(
                    commission_data(&statements),
                    STRANGER_CHECK,
                    "{}",
                    pair.label
                );
            }
        }
    }
    for pair in den_participant_pairs(world) {
        assert_same_statements(&recorded, &bob, pair.label, &pair).await;
    }
    let download = download_participant_pair(world);
    assert_same_statements(&recorded, &bob, download.label, &download).await;
}

#[tokio::test]
async fn the_recording_tells_a_node_alice_opens_from_a_miss_at_the_same_depth() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let [commission, ..] = den_stranger_pairs(&world.untitled);
    world.set_profile_cache(ProfileCacheState::Warm).await;

    let opened_mark = recorded.statements.mark();
    let opened = get(&world.alice, world.base_url(), &commission.home).await;
    assert_eq!(opened.status(), 200, "{}", commission.home);
    let opened = recorded.statements.since(opened_mark);
    let missed_mark = recorded.statements.mark();
    let missed = get(&world.alice, world.base_url(), &commission.absent).await;
    assert_eq!(missed.status(), 404, "{}", commission.absent);
    let missed = recorded.statements.since(missed_mark);

    assert_ne!(texts(&opened), texts(&missed), "{:?}", names(&opened));
}
