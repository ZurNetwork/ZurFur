//! The My Den read ports over PostgreSQL, against a throwaway container: the
//! participant listing (archived rows included) and its by-user index, a
//! commission's file list, the name-only file read, a commission's Slots and
//! an Account's Workflows, and one commission's summary. A commission's name and level
//! are its node's: the summary, the listing and the standing read them there, and a
//! commission with no node reads as absent. A node level no
//! parser reads counts as Private on every read of it. Names load as stored text, never
//! re-checked, so a row written under an older rule still reads. The summary's statement
//! answers a commission hidden from its viewer with a made-up id's row, byte for byte, never
//! reads a hidden name out of line, and probes the node and the participant once for every
//! id. One named test per read. Requires a container runtime socket.

use crate::migration_steps::{bare_pool, run_migrations_through};
use crate::node_backfill::{
    DEN_1_VERSION, OWNER_DID, seed_commission as seed_node_less, seed_owner,
};
use crate::seed;
use adapter_pg::{PgCommissionStore, PgDatabase, PgFileStore, PgPool, PgWorkflowStore};
use chrono::{SubsecRound, Utc};
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        commission::{
            CommissionFile, CommissionId, CommissionRole, CommissionStanding, CommissionSummary,
            CommissionTitle, FileKey, FileName, LockedStanding, SurfaceAddress, SurfaceName,
            ViewedSummary,
        },
        did::Did,
        handle::Handle,
        level::Level,
        node::NodeId,
        text::StoredText,
        user::{User, UserId},
        workflow::{WorkflowId, WorkflowName},
    },
    ports::{CommissionRows, CommissionStore, Database, FileStore, WorkflowStore},
};
use sqlx::{Column, PgConnection, Row, ValueRef, postgres::PgRow};

/// The shipped summary statement, as the generated reader sends it.
const FIND_SUMMARY_SQL: &str = include_str!("../../queries/commission/find_summary.sql");

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
}

/// Recognize a visitor in its own committed unit of work.
async fn provision(pool: &PgPool, did: &str) -> User {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::from(did.to_owned()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// Create and commit a commission titled `title`, owned by `owner`.
async fn seed_commission(pool: &PgPool, owner: &User, title: &str) -> CommissionId {
    seed::seed_titled(pool, &owner.id, title).await
}

/// Archive `commission`, committed.
async fn archive(pool: &PgPool, commission: CommissionId) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .set_archived(&commission, Some(Utc::now()))
        .await
        .expect("archive");
    uow.commit().await.expect("commit");
}

/// Store `token` as the level of the node `commission` points at, a test-only update.
pub(crate) async fn store_node_level(pool: &PgPool, commission: CommissionId, token: &str) {
    let updated = sqlx::query(
        "UPDATE node SET level = $1 FROM commission c WHERE c.id = $2 AND node.id = c.node_id",
    )
    .bind(token)
    .bind(uuid::Uuid::from(commission))
    .execute(pool)
    .await
    .expect("update the commission node's level");
    assert_eq!(updated.rows_affected(), 1, "one node is updated");
}

/// Store `name` as the name of the node `commission` points at, a test-only update.
async fn store_node_name(pool: &PgPool, commission: CommissionId, name: &str) {
    let updated = sqlx::query(
        "UPDATE node SET name = $1 FROM commission c WHERE c.id = $2 AND node.id = c.node_id",
    )
    .bind(name)
    .bind(uuid::Uuid::from(commission))
    .execute(pool)
    .await
    .expect("update the commission node's name");
    assert_eq!(updated.rows_affected(), 1, "one node is updated");
}

/// Drop the node table's level check in this test's own database, so a level no parser reads
/// can be stored.
pub(crate) async fn allow_any_node_level(pool: &PgPool) {
    sqlx::query("ALTER TABLE node DROP CONSTRAINT node_level_known")
        .execute(pool)
        .await
        .expect("drop node_level_known in the test database");
}

/// Asserts that `plan`, the lines of one statement's `EXPLAIN ANALYZE`, reads the `node` table
/// and runs every step it plans: no line says `never executed`. The plan is shown on failure.
#[track_caller]
pub(crate) fn assert_the_node_probe_runs(label: &str, plan: &[String]) {
    let shown = plan.join("\n");
    let reads_node = plan.iter().any(|line| {
        let words: Vec<&str> = line.split_whitespace().collect();
        words.windows(2).any(|pair| pair == ["on", "node"])
    });
    assert!(
        reads_node,
        "{label}: the plan reads no node table:\n{shown}"
    );
    let skipped = plan.iter().any(|line| line.contains("never executed"));
    assert!(!skipped, "{label}: a step of the plan never ran:\n{shown}");
}

/// Asserts that `plan`, the lines of one statement's `EXPLAIN ANALYZE`, scans
/// `commission_participant` and that every such scan ran exactly once. The plan is shown on
/// failure.
#[track_caller]
fn assert_the_participant_probe_runs_once(label: &str, plan: &[String]) {
    let shown = plan.join("\n");
    let scans: Vec<&String> = plan
        .iter()
        .filter(|line| {
            let words: Vec<&str> = line.split_whitespace().collect();
            words
                .windows(2)
                .any(|pair| pair == ["on", "commission_participant"])
        })
        .collect();
    assert!(
        !scans.is_empty(),
        "{label}: the plan scans no commission_participant table:\n{shown}"
    );
    let once = scans.iter().all(|line| line.contains("loops=1)"));
    assert!(
        once,
        "{label}: a commission_participant scan didn't run exactly once:\n{shown}"
    );
}

/// One column of a raw row: its name, and its bytes as PostgreSQL sent them, `None` when NULL.
type RawColumn = (String, Option<Vec<u8>>);

/// The one row the shipped summary statement answers for `commission` as `viewer`, each
/// column raw.
async fn raw_summary_row(
    executor: impl sqlx::PgExecutor<'_>,
    commission: uuid::Uuid,
    viewer: &str,
) -> Vec<RawColumn> {
    let rows = sqlx::query(FIND_SUMMARY_SQL)
        .bind(commission)
        .bind(viewer)
        .fetch_all(executor)
        .await
        .expect("run the summary statement");
    let [row] = <[PgRow; 1]>::try_from(rows)
        .map_err(|rows| rows.len())
        .unwrap_or_else(|count| panic!("the summary answered {count} rows, not one"));
    row.columns()
        .iter()
        .map(|column| {
            let value = row
                .try_get_raw(column.ordinal())
                .expect("read a raw column");
            let bytes = (!value.is_null()).then(|| {
                value
                    .as_bytes()
                    .expect("a non-NULL column's bytes")
                    .to_vec()
            });
            (column.name().to_owned(), bytes)
        })
        .collect()
}

/// The `node` table's TOAST blocks touched (hits and reads) as the statistics hold them once
/// `connection`'s own pending counts are flushed.
async fn flushed_node_toast_blocks(connection: &mut PgConnection) -> i64 {
    sqlx::query("SELECT pg_stat_force_next_flush()")
        .execute(&mut *connection)
        .await
        .expect("force the next statistics flush");
    sqlx::query("SELECT pg_stat_clear_snapshot()")
        .execute(&mut *connection)
        .await
        .expect("clear the statistics snapshot");
    let blocks: Option<i64> = sqlx::query_scalar(
        "SELECT toast_blks_hit + toast_blks_read FROM pg_statio_user_tables \
         WHERE relname = 'node'",
    )
    .fetch_one(&mut *connection)
    .await
    .expect("read the node table's TOAST statistics");
    blocks.expect("the node table has a TOAST table")
}

/// Add `member` as a Participant by a direct insert: no route admits a second
/// Participant yet.
async fn add_participant(pool: &PgPool, commission: CommissionId, member: &User) {
    sqlx::query(
        "INSERT INTO commission_participant (commission_id, user_id, created_at) \
         VALUES ($1, $2, now())",
    )
    .bind(uuid::Uuid::from(commission))
    .bind(member.id.as_ref())
    .execute(pool)
    .await
    .expect("insert the participant row");
}

/// Record a file link for `commission`, committed. Its time is cut to the
/// microsecond `timestamptz` stores, so it reads back equal.
async fn link_file(pool: &PgPool, commission: CommissionId, uploader: &User) -> CommissionFile {
    let file = CommissionFile {
        id: FileKey::generate(),
        commission_id: commission,
        uploaded_by: uploader.id.clone(),
        created_at: Utc::now().trunc_subsecs(6),
    };
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions().add_file(&file).await.expect("add file");
    uow.commit().await.expect("commit");
    file
}

/// The commission's one skeleton address: its only tab's only surface.
async fn only_address(pool: &PgPool, commission: CommissionId) -> SurfaceAddress {
    let composition = PgCommissionStore::new(pool.clone())
        .load_composition(&commission)
        .await
        .expect("load composition")
        .expect("every commission has its tabs");
    let surface = "content".parse::<SurfaceName>().expect("surface");
    SurfaceAddress::new(composition.tabs[0].id, surface)
}

/// Found a committed Account owned by `owner`, returning its id.
async fn seed_account(pool: &PgPool, owner: &User, handle: &str) -> AccountId {
    let (account, membership) = Account::open(
        owner.id.clone(),
        Did::from(format!("did:plc:acct-{handle}")),
        handle.parse::<Handle>().expect("handle"),
        "PG Studio".parse::<AccountName>().expect("name"),
        Utc::now(),
    );
    let id = account.id.clone();
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts()
        .create(&account, &membership)
        .await
        .expect("found account");
    uow.commit().await.expect("commit");
    id
}

/// Create a committed Workflow named `name` on `account`'s board.
async fn seed_workflow(pool: &PgPool, account: &AccountId, name: &str) -> WorkflowId {
    let board_name = name.parse::<WorkflowName>().expect("board name");
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let workflow = uow
        .workflows()
        .create(&board_name, account)
        .await
        .expect("create the board");
    uow.commit().await.expect("commit");
    workflow.id
}

// The participant listing reads the participant record: the owner and a
// seated member each list the commissions they take part in, archived ones
// included and marked; a stranger lists none. Ordered by id.
#[tokio::test]
async fn participant_listing_includes_archived_commissions() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:readsowner").await;
    let member = provision(&pool, "did:plc:readsmember").await;
    let stranger = provision(&pool, "did:plc:readsstranger").await;
    let archived = seed_commission(&pool, &owner, "Old sketch").await;
    let live = seed_commission(&pool, &owner, "Untitled").await;
    archive(&pool, archived).await;
    add_participant(&pool, live, &member).await;
    let store = PgCommissionStore::new(pool.clone());

    let owners = store.list_participating(&owner.id).await.expect("list");
    let members = store.list_participating(&member.id).await.expect("list");
    let strangers = store.list_participating(&stranger.id).await.expect("list");

    let owners_ids: Vec<CommissionId> = owners.iter().map(|summary| summary.id).collect();
    assert_eq!(owners_ids, [archived, live]);
    assert_eq!(owners[0].title.as_str(), "Old sketch");
    assert!(
        owners[0].is_archived(),
        "the archived commission stays listed"
    );
    assert!(!owners[1].is_archived());
    assert!(
        owners
            .iter()
            .all(|summary| summary.visibility == Level::Private)
    );
    let members_ids: Vec<CommissionId> = members.iter().map(|summary| summary.id).collect();
    assert_eq!(members_ids, [live]);
    assert!(strangers.is_empty());
}

// The participant table carries its by-user index, user first, so the listing
// seeks it instead of scanning every membership.
#[tokio::test]
async fn the_participant_table_has_its_by_user_index() {
    let (pool, _container) = fresh_pool().await;

    let definition: Option<String> = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes \
         WHERE tablename = 'commission_participant' \
           AND indexname = 'commission_participant_by_user'",
    )
    .fetch_optional(&pool)
    .await
    .expect("read pg_indexes");

    let definition = definition.expect("the by-user index exists");
    assert!(
        definition.contains("(user_id, commission_id)"),
        "user first, then commission: {definition}"
    );
}

// A stored name today's rule refuses still loads, verbatim, through the
// listing and through the summary: loads never re-check a name.
#[tokio::test]
async fn a_stored_title_loads_without_a_recheck() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:readslegacy").await;
    let id = seed_commission(&pool, &owner, "Legacy").await;
    store_node_name(&pool, id, "   ").await;
    assert!("   ".parse::<CommissionTitle>().is_err());
    let store = PgCommissionStore::new(pool.clone());

    let listing = store.list_participating(&owner.id).await.expect("list");
    let found = store
        .find_summary(&id, &owner.id)
        .await
        .expect("find_summary")
        .expect("present");

    assert_eq!(listing[0].title.as_str(), "   ");
    assert_eq!(found.summary.title.as_str(), "   ");
}

// The file list holds only this commission's entries, in key (upload) order;
// an unknown commission has none.
#[tokio::test]
async fn file_list_holds_only_this_commissions_entries() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:readsfiles").await;
    let mine = seed_commission(&pool, &owner, "Mine").await;
    let other = seed_commission(&pool, &owner, "Other").await;
    let first = link_file(&pool, mine, &owner).await;
    link_file(&pool, other, &owner).await;
    let second = link_file(&pool, mine, &owner).await;
    let store = PgCommissionStore::new(pool.clone());

    let files = store.files(&mine).await.expect("files");
    let unknown = CommissionId::from(uuid::Uuid::now_v7());

    let keys: Vec<FileKey> = files.iter().map(|file| file.id).collect();
    assert_eq!(keys, [first.id, second.id]);
    let upload_times: Vec<_> = files.iter().map(|file| file.created_at).collect();
    assert_eq!(upload_times, [first.created_at, second.created_at]);
    assert!(store.files(&unknown).await.expect("files").is_empty());
}

// The name read answers the stored filename without the bytes, `None` for a
// missing key, and a name stored under an older rule loads verbatim, there
// and on the download.
#[tokio::test]
async fn filename_read_answers_the_stored_name() {
    let (pool, _container) = fresh_pool().await;
    let store = PgFileStore::new(pool.clone());
    let key = FileKey::generate();
    let legacy_key = FileKey::generate();
    let name = FileName::try_from("ref-abco.png").expect("filename");
    let mut content: &[u8] = b"bytes";
    store
        .put(key, &name, "image/png", &mut content)
        .await
        .expect("put");
    let mut legacy_content: &[u8] = b"bytes";
    store
        .put(legacy_key, &name, "image/png", &mut legacy_content)
        .await
        .expect("put");
    sqlx::query("UPDATE file_blob SET filename = $1 WHERE key = $2")
        .bind("a\u{1}b.png")
        .bind(uuid::Uuid::from(legacy_key))
        .execute(&pool)
        .await
        .expect("store a filename today's rule refuses");
    assert!(FileName::try_from("a\u{1}b.png").is_err());

    let stored = store.filename(key).await.expect("filename");
    let missing = store.filename(FileKey::generate()).await.expect("filename");
    let legacy = store.filename(legacy_key).await.expect("filename");
    let legacy_download = store.get(legacy_key).await.expect("get").expect("present");

    assert_eq!(stored.expect("present").as_str(), "ref-abco.png");
    assert!(missing.is_none());
    assert_eq!(legacy.expect("present").as_str(), "a\u{1}b.png");
    assert_eq!(legacy_download.metadata.filename.as_str(), "a\u{1}b.png");
}

// The Slot list holds this commission's Slots in declaration order and
// nothing of another commission's.
#[tokio::test]
async fn slot_list_holds_this_commissions_slots_in_declaration_order() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:readsslots").await;
    let mine = seed_commission(&pool, &owner, "Mine").await;
    let other = seed_commission(&pool, &owner, "Other").await;
    let my_address = only_address(&pool, mine).await;
    let other_address = only_address(&pool, other).await;
    crate::seed::declare_committed(&pool, mine, &owner.id, &my_address, "Abco", None).await;
    crate::seed::declare_committed(&pool, other, &owner.id, &other_address, "Elsewhere", None)
        .await;
    crate::seed::declare_committed(&pool, mine, &owner.id, &my_address, "Ember", None).await;
    let store = PgCommissionStore::new(pool.clone());

    let listed = store.slots(&mine).await.expect("slots");

    let titles: Vec<&str> = listed.iter().map(|slot| slot.title.as_str()).collect();
    assert_eq!(titles, ["Abco", "Ember"]);
    assert!(listed.iter().all(|slot| slot.commission_id == mine));
}

// The Workflow listing holds this Account's boards only, by id; an Account
// with none lists nothing.
#[tokio::test]
async fn workflow_listing_holds_only_this_accounts_workflows() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:readsworkflows").await;
    let mine = seed_account(&pool, &owner, "readsmine.zurfur.app").await;
    let other = seed_account(&pool, &owner, "readsother.zurfur.app").await;
    let empty = seed_account(&pool, &owner, "readsempty.zurfur.app").await;
    let queue = seed_workflow(&pool, &mine, "Queue").await;
    seed_workflow(&pool, &other, "Elsewhere").await;
    let backlog = seed_workflow(&pool, &mine, "Backlog").await;
    let store = PgWorkflowStore::new(pool.clone());

    let listing = store.list_for_account(&mine).await.expect("list");
    let nothing = store.list_for_account(&empty).await.expect("list");

    let listed: Vec<(WorkflowId, &str)> = listing
        .iter()
        .map(|summary| (summary.id, summary.name.as_str()))
        .collect();
    assert_eq!(listed, [(queue, "Queue"), (backlog, "Backlog")]);
    assert!(nothing.is_empty());
}

// The summary read answers `None` for an id no commission carries.
#[tokio::test]
async fn the_summary_of_a_missing_commission_is_none() {
    let (pool, _container) = fresh_pool().await;
    let store = PgCommissionStore::new(pool.clone());
    let missing = CommissionId::from(uuid::Uuid::now_v7());
    let viewer = UserId::from(Did::from("did:plc:summarymissingviewer".to_owned()));

    let summary = store
        .find_summary(&missing, &viewer)
        .await
        .expect("find_summary");

    assert_eq!(summary, None);
}

// The shipped summary statement answers a stranger, who owns nothing and takes part in
// nothing, with a made-up id's row, every column's bytes and NULLs alike, for each cause a
// commission is hidden: Private, archived, an unreadable level, and no node (on a database
// migrated through the node table's migration). A Listed live commission's row differs.
#[tokio::test]
async fn every_hidden_commission_s_summary_row_is_an_absent_id_s_byte_for_byte() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:hiddenrowowner").await;
    let stranger = provision(&pool, "did:plc:hiddenrowstranger").await;
    let private_live = seed_commission(&pool, &owner, "Private live").await;
    let private_archived = seed_commission(&pool, &owner, "Private archived").await;
    archive(&pool, private_archived).await;
    let listed_archived = seed_commission(&pool, &owner, "Listed archived").await;
    store_node_level(&pool, listed_archived, "listed").await;
    archive(&pool, listed_archived).await;
    let public_archived = seed_commission(&pool, &owner, "Public archived").await;
    store_node_level(&pool, public_archived, "public").await;
    archive(&pool, public_archived).await;
    allow_any_node_level(&pool).await;
    let unreadable_live = seed_commission(&pool, &owner, "Unreadable live").await;
    store_node_level(&pool, unreadable_live, "not-a-visibility").await;
    let unreadable_archived = seed_commission(&pool, &owner, "Unreadable archived").await;
    store_node_level(&pool, unreadable_archived, "not-a-visibility").await;
    archive(&pool, unreadable_archived).await;
    assert!("not-a-visibility".parse::<Level>().is_err());
    let listed_live = seed_commission(&pool, &owner, "Listed live").await;
    store_node_level(&pool, listed_live, "listed").await;
    let viewer = stranger.id.as_ref();

    let absent_row = raw_summary_row(&pool, uuid::Uuid::now_v7(), viewer).await;
    let hidden = [
        ("Private, live", private_live),
        ("Private, archived", private_archived),
        ("Listed, archived", listed_archived),
        ("Public, archived", public_archived),
        ("an unreadable level, live", unreadable_live),
        ("an unreadable level, archived", unreadable_archived),
    ];
    for (label, commission) in hidden {
        let hidden_row = raw_summary_row(&pool, uuid::Uuid::from(commission), viewer).await;
        assert_eq!(
            hidden_row, absent_row,
            "{label}: the hidden row is an absent id's"
        );
    }
    let shown_row = raw_summary_row(&pool, uuid::Uuid::from(listed_live), viewer).await;
    assert_ne!(shown_row, absent_row, "a Listed live commission is shown");

    let (den_pool, _den_container) = bare_pool().await;
    run_migrations_through(&den_pool, DEN_1_VERSION).await;
    seed_owner(&den_pool).await;
    let node_less = seed_node_less(&den_pool, "No node", "public").await;
    let den_absent_row = raw_summary_row(&den_pool, uuid::Uuid::now_v7(), viewer).await;
    let node_less_row = raw_summary_row(&den_pool, node_less, viewer).await;
    assert_eq!(
        node_less_row, den_absent_row,
        "a commission with no node: the hidden row is an absent id's"
    );
    assert_eq!(
        den_absent_row, absent_row,
        "an absent id answers the same row on either schema"
    );
}

// A commission's name stored out of line (2048 bytes that don't compress) is never read from
// the node table's TOAST table while the summary hides it: a stranger's read leaves the
// table's TOAST block count where it was, and its owner's read, which shows the name, raises
// it. Every read and statistics flush runs on one connection.
#[tokio::test]
async fn a_hidden_name_is_never_detoasted() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:detoastowner").await;
    let stranger = provision(&pool, "did:plc:detoaststranger").await;
    let id = seed_commission(&pool, &owner, "Untitled").await;
    let mut connection = pool.acquire().await.expect("acquire a connection");
    // No autovacuum pass may touch the TOAST table between two counts.
    sqlx::query(
        "ALTER TABLE node SET (autovacuum_enabled = false, toast.autovacuum_enabled = false)",
    )
    .execute(&mut *connection)
    .await
    .expect("turn autovacuum off for node in the test database");
    let renamed = sqlx::query(
        "UPDATE node \
         SET name = (SELECT string_agg(md5(i::text), '' ORDER BY i) \
                     FROM generate_series(1, 64) AS i) \
         FROM commission c WHERE c.id = $1 AND node.id = c.node_id",
    )
    .bind(uuid::Uuid::from(id))
    .execute(&mut *connection)
    .await
    .expect("store a long name on the commission's node");
    assert_eq!(renamed.rows_affected(), 1, "one node is renamed");
    let (length, compression): (i32, Option<String>) = sqlx::query_as(
        "SELECT octet_length(node.name), pg_column_compression(node.name) \
         FROM node JOIN commission c ON node.id = c.node_id WHERE c.id = $1",
    )
    .bind(uuid::Uuid::from(id))
    .fetch_one(&mut *connection)
    .await
    .expect("read the stored name's size");
    assert_eq!(length, 2048, "the name is 2048 bytes");
    assert_eq!(compression, None, "the name is stored uncompressed");
    let toast_table: String = sqlx::query_scalar(
        "SELECT reltoastrelid::regclass::text FROM pg_class WHERE oid = 'node'::regclass",
    )
    .fetch_one(&mut *connection)
    .await
    .expect("name the node table's TOAST table");
    let count_chunks = format!("SELECT count(*) FROM {toast_table}");
    let chunks: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(count_chunks))
        .fetch_one(&mut *connection)
        .await
        .expect("count the TOAST chunks");
    assert!(chunks > 0, "the name is stored out of line");

    let before_hidden = flushed_node_toast_blocks(&mut connection).await;
    raw_summary_row(&mut *connection, uuid::Uuid::from(id), stranger.id.as_ref()).await;
    let after_hidden = flushed_node_toast_blocks(&mut connection).await;
    let shown_row =
        raw_summary_row(&mut *connection, uuid::Uuid::from(id), owner.id.as_ref()).await;
    let after_shown = flushed_node_toast_blocks(&mut connection).await;

    assert_eq!(
        after_hidden, before_hidden,
        "the stranger's read touches no TOAST block"
    );
    assert!(
        after_shown > after_hidden,
        "the owner's read detoasts the name: {after_hidden} -> {after_shown}"
    );
    let shown_name = shown_row
        .iter()
        .find(|(column, _)| column == "name")
        .and_then(|(_, bytes)| bytes.as_ref())
        .expect("the owner's row holds the name");
    assert_eq!(shown_name.len(), 2048, "the owner reads the whole name");
}

// The shipped summary statement probes the node index and the participant table once each, and
// runs every step it plans, for a made-up id, a stranger on a Private commission, a Participant
// on it and its owner alike.
#[tokio::test]
async fn the_summary_probes_the_node_and_the_participant_once_for_every_id() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:summaryplanowner").await;
    let member = provision(&pool, "did:plc:summaryplanmember").await;
    let stranger = provision(&pool, "did:plc:summaryplanstranger").await;
    let private = seed_commission(&pool, &owner, "Untitled").await;
    add_participant(&pool, private, &member).await;
    let made_up = CommissionId::from(uuid::Uuid::now_v7());
    let explain = format!("EXPLAIN (ANALYZE, COSTS OFF, TIMING OFF) {FIND_SUMMARY_SQL}");

    let cases = [
        ("a made-up id", made_up, &stranger),
        ("a stranger on a Private commission", private, &stranger),
        ("a Participant on it", private, &member),
        ("its owner", private, &owner),
    ];
    for (label, target, viewer) in cases {
        let plan: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(explain.clone()))
            .bind(uuid::Uuid::from(target))
            .bind(viewer.id.as_ref())
            .fetch_all(&pool)
            .await
            .expect("explain the summary statement");
        assert_the_node_probe_runs(label, &plan);
        assert_the_participant_probe_runs_once(label, &plan);
    }
}

// The summary read answers the title as stored, the visibility and whether it
// is archived, for a live commission and an archived one alike.
#[tokio::test]
async fn the_summary_answers_the_stored_title_visibility_and_archived_flag() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:summaryowner").await;
    let live = seed_commission(&pool, &owner, "Untitled").await;
    let archived = seed_commission(&pool, &owner, "Old sketch").await;
    archive(&pool, archived).await;
    store_node_level(&pool, live, "listed").await;
    let store = PgCommissionStore::new(pool.clone());

    let live_summary = store
        .find_summary(&live, &owner.id)
        .await
        .expect("find_summary");
    let archived_summary = store
        .find_summary(&archived, &owner.id)
        .await
        .expect("find_summary");

    let expected_live = CommissionSummary {
        id: live,
        title: StoredText::from("Untitled".to_owned()),
        visibility: Level::Listed,
        archived: false,
    };
    let expected_archived = CommissionSummary {
        id: archived,
        title: StoredText::from("Old sketch".to_owned()),
        visibility: Level::Private,
        archived: true,
    };
    let viewed_live = ViewedSummary {
        summary: expected_live,
        role: Some(CommissionRole::Owner),
    };
    let viewed_archived = ViewedSummary {
        summary: expected_archived,
        role: Some(CommissionRole::Owner),
    };
    assert_eq!(live_summary, Some(viewed_live));
    assert_eq!(archived_summary, Some(viewed_archived));
}

// A node level no parser reads (stored with `node_level_known` dropped in this test's own
// database) counts as Private: the summary, the standing and the participant listing each
// read it so, and none fails, the whole commission's read included. A Participant reads its
// summary at Private, and a stranger, shown no Private commission, reads it as absent.
#[tokio::test]
async fn an_unreadable_node_level_reads_private() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:unreadableowner").await;
    let member = provision(&pool, "did:plc:unreadablemember").await;
    let stranger = provision(&pool, "did:plc:unreadablestranger").await;
    let id = seed_commission(&pool, &owner, "Untitled").await;
    add_participant(&pool, id, &member).await;
    allow_any_node_level(&pool).await;
    store_node_level(&pool, id, "not-a-visibility").await;
    assert!("not-a-visibility".parse::<Level>().is_err());
    let store = PgCommissionStore::new(pool.clone());

    let summary = store
        .find_summary(&id, &owner.id)
        .await
        .expect("find_summary");
    let member_summary = store
        .find_summary(&id, &member.id)
        .await
        .expect("find_summary");
    let stranger_summary = store
        .find_summary(&id, &stranger.id)
        .await
        .expect("find_summary");
    let standing = store.standing(&id, &owner.id).await.expect("standing");
    let found = store.find(&id).await.expect("find");
    let listing = store
        .list_participating(&owner.id)
        .await
        .expect("list_participating");

    let summary = summary.expect("present");
    let member_summary = member_summary.expect("present to a Participant");
    let standing = standing.expect("present");
    assert!(found.is_some(), "the whole commission still reads");
    assert_eq!(summary.summary.visibility, Level::Private);
    assert_eq!(member_summary.summary.visibility, Level::Private);
    assert_eq!(member_summary.role, Some(CommissionRole::Participant));
    assert_eq!(stranger_summary, None, "a stranger reads it as absent");
    assert_eq!(standing.level, Level::Private);
    let listed: Vec<(CommissionId, Level)> = listing
        .into_iter()
        .map(|summary| (summary.id, summary.visibility))
        .collect();
    assert_eq!(listed, [(id, Level::Private)]);
}

// The summary, the participant listing and the standing read the name and level of the node
// the commission points at, which test-only updates have set apart from what it was created
// with.
#[tokio::test]
async fn the_summary_and_standing_read_the_node_s_name_and_level() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:nodereadsowner").await;
    let id = seed_commission(&pool, &owner, "Untitled").await;
    store_node_level(&pool, id, "public").await;
    store_node_name(&pool, id, "The node's name").await;
    let store = PgCommissionStore::new(pool.clone());

    let viewed = store
        .find_summary(&id, &owner.id)
        .await
        .expect("find_summary");
    let listing = store
        .list_participating(&owner.id)
        .await
        .expect("list_participating");
    let standing = store.standing(&id, &owner.id).await.expect("standing");
    let database = PgDatabase::new(pool.clone());
    let mut unit = database.begin().await.expect("begin");
    let locked = unit
        .commissions()
        .standing_for_update(&id, &owner.id)
        .await
        .expect("standing_for_update");

    let expected_summary = CommissionSummary {
        id,
        title: StoredText::from("The node's name".to_owned()),
        visibility: Level::Public,
        archived: false,
    };
    let expected_standing = CommissionStanding {
        role: Some(CommissionRole::Owner),
        level: Level::Public,
        archived: false,
    };
    let expected_viewed = ViewedSummary {
        summary: expected_summary.clone(),
        role: Some(CommissionRole::Owner),
    };
    assert_eq!(viewed, Some(expected_viewed));
    assert_eq!(listing, [expected_summary]);
    let node: uuid::Uuid = sqlx::query_scalar("SELECT node_id FROM commission WHERE id = $1")
        .bind(uuid::Uuid::from(id))
        .fetch_one(&pool)
        .await
        .expect("read the commission's node");
    let expected_locked = LockedStanding {
        standing: expected_standing,
        node: NodeId::from(node),
    };
    assert_eq!(standing, Some(expected_standing));
    assert_eq!(locked, Some(expected_locked));
}

// On a database migrated only through the node table's migration, a commission row with no
// node is absent to the summary, the participant listing and the locked standing, and the
// pool's standing finds it at Private, whatever visibility that schema's row was seeded with.
#[tokio::test]
async fn a_node_less_commission_reads_as_absent_and_its_standing_as_private() {
    let (pool, _container) = bare_pool().await;
    run_migrations_through(&pool, DEN_1_VERSION).await;
    seed_owner(&pool).await;
    let id = CommissionId::from(seed_node_less(&pool, "No node", "public").await);
    sqlx::query(
        "INSERT INTO commission_participant (commission_id, user_id, created_at) \
         VALUES ($1, $2, now())",
    )
    .bind(uuid::Uuid::from(id))
    .bind(OWNER_DID)
    .execute(&pool)
    .await
    .expect("insert the owner's participant row");
    let owner = UserId::from(Did::from(OWNER_DID.to_owned()));
    let store = PgCommissionStore::new(pool.clone());

    let summary = store.find_summary(&id, &owner).await.expect("find_summary");
    let listing = store
        .list_participating(&owner)
        .await
        .expect("list_participating");
    let standing = store.standing(&id, &owner).await.expect("standing");
    let database = PgDatabase::new(pool.clone());
    let mut unit = database.begin().await.expect("begin");
    let locked = unit
        .commissions()
        .standing_for_update(&id, &owner)
        .await
        .expect("standing_for_update");

    let expected_standing = CommissionStanding {
        role: Some(CommissionRole::Owner),
        level: Level::Private,
        archived: false,
    };
    assert_eq!(summary, None, "the summary reads it as absent");
    assert!(listing.is_empty(), "the listing leaves it out: {listing:?}");
    assert_eq!(standing, Some(expected_standing), "the standing");
    assert_eq!(locked, None, "the locked standing reads it as absent");
}

// An archive time chrono can't hold (`infinity`, by a test-only update) never
// reaches a decoder: the summary and the participant listing each read the
// commission as archived, and neither panics.
#[tokio::test]
async fn an_archive_time_out_of_range_reads_as_archived_without_a_panic() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:infinityowner").await;
    let id = seed_commission(&pool, &owner, "Old sketch").await;
    let updated = sqlx::query("UPDATE commission SET archived_at = 'infinity' WHERE id = $1")
        .bind(uuid::Uuid::from(id))
        .execute(&pool)
        .await
        .expect("store an archive time out of chrono's range");
    assert_eq!(updated.rows_affected(), 1, "one commission row is updated");
    let store = PgCommissionStore::new(pool.clone());

    let viewed = store
        .find_summary(&id, &owner.id)
        .await
        .expect("find_summary");
    let listing = store
        .list_participating(&owner.id)
        .await
        .expect("list_participating");

    assert!(viewed.expect("present").summary.is_archived());
    let listed: Vec<(CommissionId, bool)> = listing
        .iter()
        .map(|summary| (summary.id, summary.is_archived()))
        .collect();
    assert_eq!(listed, [(id, true)]);
}
