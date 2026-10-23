//! Each pair of not-found causes at the same depth (absent; present but not the viewer's) runs
//! the same SQL statements in the same order on PostgreSQL, through the den routes and through
//! commission's reads behind den's check, with the profile cache warm and then cold, and
//! through the rename and the board card's placement. The statements are read off the wire by
//! the recording proxy; only bound values may differ.
//! Requires a container runtime socket.

use domain::elements::{did::Did, level::Level};
use strum::VariantArray as _;
use test_support::{
    den::{get, patch_title},
    pg_app::{
        self, COMMISSION_NOT_FOUND, DenWorld, MissPair, ProfileCacheState, RENAME_TITLE, Statement,
        StatementLog, absent_commission, archived_participant_pairs, commission_read_live_pairs,
        commission_read_stranger_pairs, den_account_pair, den_character_pair,
        den_participant_pairs, den_stranger_pairs, den_stranger_targets, download_participant_pair,
        rename_stranger_pair, stranger_targets,
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

/// What placing a card reads of a commission the actor can't see, and all of it: the
/// actor's standing, then the actor's own view grant.
const BOARD_CARD_CHECK: [&str; 2] = ["commission/standing", "commission/view_grant"];

/// Everything a rename by someone outside the commission runs, and all of it: the session's
/// load, then the actor's standing, read on the pool before any unit opens.
const RENAME_MISS: [&str; 2] = ["session/load", "commission/standing"];

/// The profile cache's read, by DID.
const PROFILE_GET: &str = "profile/get";

/// The profile cache's fill, after a lookup.
const PROFILE_PUT: &str = "profile/put";

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
fn stranger(world: &DenWorld) -> Reader<'_> {
    let did = pg_app::stranger().did().clone();
    Reader {
        client: &world.stranger,
        did,
    }
}

/// bob, as a reader.
fn bob(world: &DenWorld) -> Reader<'_> {
    let did = adapter_mem::fixture::bob().did().clone();
    Reader {
        client: &world.bob,
        did,
    }
}

/// Whether a read names the viewer's root, which a cold profile cache makes it look up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Root {
    /// The den routes: every answer names the root.
    Named,
    /// Commission's reads: no answer names it, so no profile is read.
    NotNamed,
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
    let statements = recorded.statements.since(mark);
    let unreadable = statements.iter().any(Statement::is_unreadable);
    assert!(!unreadable, "{label}, {cache}: {:?}", names(&statements));
    statements
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

/// Asserts each cause of `pair` runs the same statements as `reader`, from each cache state;
/// that every profile statement reads the reader's own DID; and, where the read names the
/// root, that only the cold runs look its name up. Returns the absent cause's statements, by
/// cache state.
async fn assert_same_statements(
    recorded: &Recorded,
    reader: &Reader<'_>,
    root: Root,
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
        assert_the_cache_state_shows(&absent, root, cache, label);
        by_state.push(absent);
    }
    by_state
}

/// Asserts `statements` read the profile cache as `cache` says for a read whose `root` is as
/// given: a root named from a warm cache only reads it, from a cold one also fills it after
/// the lookup; a read that names no root reads no profile at all.
fn assert_the_cache_state_shows(
    statements: &[Statement],
    root: Root,
    cache: ProfileCacheState,
    label: &str,
) {
    let ran = names(statements);
    let profile_reads: Vec<&str> = ran
        .iter()
        .map(String::as_str)
        .filter(|name| name.starts_with("profile/"))
        .collect();
    let expected: &[&str] = match (root, cache) {
        (Root::Named, ProfileCacheState::Warm) => &[PROFILE_GET],
        (Root::Named, ProfileCacheState::Cold) => &[PROFILE_GET, PROFILE_PUT],
        (Root::NotNamed, _) => &[],
    };
    assert_eq!(profile_reads, expected, "{label}, {cache}: {ran:?}");
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
    let stranger = stranger(world);

    for (target, visibility) in den_stranger_targets(world) {
        world.set_visibility(target.id, visibility).await;
        let lifecycle = world.archived_or_live(&target);
        for pair in den_stranger_pairs(&target) {
            let label = format!("{visibility} {lifecycle}, {}", pair.label);

            let runs =
                assert_same_statements(&recorded, &stranger, Root::Named, &label, &pair).await;

            for statements in runs {
                assert_runs_none_of(&statements, &[WHOLE_COMMISSION], &label);
            }
        }
    }
    let account = den_account_pair(world);
    assert_same_statements(&recorded, &stranger, Root::Named, account.label, &account).await;
}

#[tokio::test]
async fn a_stranger_s_unknown_did_and_another_user_s_run_the_same_statements_under_characters() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let stranger = stranger(world);
    let characters = den_character_pair();

    assert_same_statements(
        &recorded,
        &stranger,
        Root::Named,
        characters.label,
        &characters,
    )
    .await;
}

#[tokio::test]
async fn a_participant_s_causes_run_the_same_statements_through_the_den_routes() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let bob = bob(world);

    for pair in den_participant_pairs(world) {
        let runs = assert_same_statements(&recorded, &bob, Root::Named, pair.label, &pair).await;

        for statements in runs {
            assert_runs_none_of(&statements, &[WORKFLOW_BY_KEY], pair.label);
        }
    }
}

#[tokio::test]
async fn a_stranger_s_causes_run_the_same_statements_through_commission_s_reads() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let stranger = stranger(world);

    for (target, visibility) in stranger_targets(world) {
        world.set_visibility(target.id, visibility).await;
        let lifecycle = world.archived_or_live(&target);
        for pair in commission_read_stranger_pairs(&target) {
            let label = format!("{visibility} {lifecycle}, {}", pair.label);

            let runs =
                assert_same_statements(&recorded, &stranger, Root::NotNamed, &label, &pair).await;

            for statements in runs {
                assert_eq!(commission_data(&statements), STRANGER_CHECK, "{label}");
            }
        }
    }
    for (visibility, pair, code) in commission_read_live_pairs(world) {
        world.set_visibility(world.untitled.id, visibility).await;
        let label = format!("{visibility}, live, {}", pair.label);

        let runs =
            assert_same_statements(&recorded, &stranger, Root::NotNamed, &label, &pair).await;

        for statements in runs {
            if code == COMMISSION_NOT_FOUND {
                assert_eq!(commission_data(&statements), STRANGER_CHECK, "{label}");
            }
            assert_runs_none_of(&statements, &NEVER_FOR_A_STRANGER, &label);
        }
    }
}

#[tokio::test]
async fn a_participant_s_causes_under_an_archived_commission_run_the_same_statements() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    world.add_bob_to_old_sketch().await;
    let bob = bob(world);
    let old_sketch = world.old_sketch;
    let his_own = [
        format!(
            "/den/commissions/{}/attachments/{}",
            old_sketch.id, old_sketch.file
        ),
        format!(
            "/den/commissions/{}/slots/{}",
            old_sketch.id, old_sketch.slot
        ),
        format!("/commissions/{}/files/{}", old_sketch.id, old_sketch.file),
    ];
    for path in his_own {
        let opened = get(bob.client, world.base_url(), &path).await;
        assert_eq!(opened.status(), 200, "bob opens his own {path}");
    }
    let [den_file, den_slot, download] = archived_participant_pairs(world);

    for pair in [den_file, den_slot] {
        assert_same_statements(&recorded, &bob, Root::Named, pair.label, &pair).await;
    }
    let runs =
        assert_same_statements(&recorded, &bob, Root::NotNamed, download.label, &download).await;
    for statements in runs {
        assert_runs_none_of(&statements, &NEVER_FOR_A_MISSING_FILE, download.label);
    }
}

#[tokio::test]
async fn a_participant_s_causes_run_the_same_statements_through_the_download() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let bob = bob(world);
    let pair = download_participant_pair(world);

    let runs = assert_same_statements(&recorded, &bob, Root::NotNamed, pair.label, &pair).await;

    for statements in runs {
        assert_runs_none_of(&statements, &NEVER_FOR_A_MISSING_FILE, pair.label);
    }
}

#[tokio::test]
async fn with_a_visibility_no_parser_reads_each_pair_still_runs_the_same_statements() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let stranger = stranger(world);
    let bob = bob(world);
    world.store_unparsable_level(world.old_sketch.id).await;
    world.store_unparsable_level(world.untitled.id).await;

    for target in [world.old_sketch, world.untitled] {
        for pair in den_stranger_pairs(&target) {
            let runs =
                assert_same_statements(&recorded, &stranger, Root::Named, pair.label, &pair).await;

            for statements in runs {
                assert_runs_none_of(&statements, &[WHOLE_COMMISSION], pair.label);
            }
        }
        for pair in commission_read_stranger_pairs(&target) {
            let runs =
                assert_same_statements(&recorded, &stranger, Root::NotNamed, pair.label, &pair)
                    .await;

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
        assert_same_statements(&recorded, &bob, Root::Named, pair.label, &pair).await;
    }
    let download = download_participant_pair(world);
    assert_same_statements(&recorded, &bob, Root::NotNamed, download.label, &download).await;
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

/// The statements `POST /commissions/{commission}/placements` into `column` runs as `reader`,
/// after checking it is answered `status`.
async fn placement_statements(
    recorded: &Recorded,
    reader: &Reader<'_>,
    commission: domain::elements::commission::CommissionId,
    column: domain::elements::workflow::ColumnId,
    status: u16,
) -> Vec<Statement> {
    let path = format!("/commissions/{commission}/placements");
    let body = serde_json::json!({ "column_id": column.to_string(), "index": 0 });
    let mark = recorded.statements.mark();
    let response = reader
        .client
        .post(format!("{}{path}", recorded.world.base_url()))
        .json(&body)
        .send()
        .await
        .expect("the placement is answered");
    assert_eq!(response.status(), status, "{path}");
    let statements = recorded.statements.since(mark);
    let unreadable = statements.iter().any(Statement::is_unreadable);
    assert!(!unreadable, "{path}: {:?}", names(&statements));
    statements
}

#[tokio::test]
async fn a_stranger_s_board_card_and_a_made_up_id_s_run_the_same_statements() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let bob = bob(world);
    let column = world.add_backlog_column("To do").await;

    // bob owns "Night Shift"'s board and holds no role and no grant on the Private "Old sketch".
    let not_yours = placement_statements(&recorded, &bob, world.old_sketch.id, column, 404).await;
    let absent = placement_statements(&recorded, &bob, absent_commission(), column, 404).await;

    assert_eq!(
        texts(&absent),
        texts(&not_yours),
        "{:?} against {:?}",
        names(&absent),
        names(&not_yours)
    );
    assert_eq!(commission_data(&absent), BOARD_CARD_CHECK);
    assert_eq!(commission_data(&not_yours), BOARD_CARD_CHECK);
}

// A Participant's card of a Private commission passes on the role, after the same commission
// reads a miss takes: the standing, then the view grant.
#[tokio::test]
async fn a_participant_s_board_card_reads_what_a_miss_reads() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let bob = bob(world);
    let column = world.add_backlog_column("To do").await;

    let placed = placement_statements(&recorded, &bob, world.untitled.id, column, 204).await;

    assert_eq!(commission_data(&placed), BOARD_CARD_CHECK);
}

/// The statements `PATCH {path}` with [`RENAME_TITLE`] runs as `reader`, after checking it is a
/// `404`.
async fn rename_statements(recorded: &Recorded, reader: &Reader<'_>, path: &str) -> Vec<Statement> {
    let mark = recorded.statements.mark();
    let response = patch_title(reader.client, recorded.world.base_url(), path, RENAME_TITLE).await;
    assert_eq!(response.status(), 404, "{path}");
    let statements = recorded.statements.since(mark);
    let unreadable = statements.iter().any(Statement::is_unreadable);
    assert!(!unreadable, "{path}: {:?}", names(&statements));
    statements
}

/// Whether `statement` opens a transaction.
fn begins(statement: &Statement) -> bool {
    let words = statement.sql.trim_start().to_ascii_uppercase();
    words.starts_with("BEGIN") || words.starts_with("START TRANSACTION")
}

#[tokio::test]
async fn a_stranger_s_rename_and_a_made_up_id_s_run_the_same_statements() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let bob = bob(world);
    let pair = rename_stranger_pair(world);

    // bob takes no part in "Old sketch".
    let not_yours = rename_statements(&recorded, &bob, &pair.not_yours).await;
    let absent = rename_statements(&recorded, &bob, &pair.absent).await;

    for (cause, statements) in [("not yours", &not_yours), ("absent", &absent)] {
        assert_eq!(names(statements), RENAME_MISS, "{cause}");
        let opened = statements.iter().find(|statement| begins(statement));
        assert_eq!(opened, None, "{cause}: a unit opened");
    }
    assert_eq!(texts(&absent), texts(&not_yours));
}

/// Everything the commission read runs for someone it doesn't open to, and all of it: the
/// session's load, then den's check, the narrow projection and the viewer's participant row.
const GET_COMMISSION_MISS: [&str; 3] = [
    "session/load",
    "commission/find_summary",
    "commission/is_participant",
];

#[tokio::test]
async fn get_commission_runs_the_check_s_pair_alike_for_a_stranger_and_a_made_up_id() {
    let recorded = recorded_world().await;
    let world = &recorded.world;
    let stranger = stranger(world);
    let absent = format!("/commissions/{}", absent_commission());
    let mut targets = stranger_targets(world).to_vec();
    targets.push((world.untitled, Level::Listed));

    for (target, visibility) in targets {
        world.set_visibility(target.id, visibility).await;
        let lifecycle = world.archived_or_live(&target);
        let label = format!("{visibility} {lifecycle}, the commission read");
        let pair = MissPair {
            label: "the commission read",
            absent: absent.clone(),
            not_yours: format!("/commissions/{}", target.id),
            home: format!("/commissions/{}", target.id),
        };

        let runs =
            assert_same_statements(&recorded, &stranger, Root::NotNamed, &label, &pair).await;

        for statements in runs {
            assert_eq!(names(&statements), GET_COMMISSION_MISS, "{label}");
            assert_runs_none_of(&statements, &[WHOLE_COMMISSION], &label);
        }
    }
}
