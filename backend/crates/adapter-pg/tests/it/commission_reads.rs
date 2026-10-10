//! The My Den read ports over PostgreSQL, against a throwaway container: the
//! participant listing (archived rows included) and its by-user index, a
//! commission's file list, the name-only file read, a commission's Slots and
//! an Account's Workflows. Names load as stored text, never re-checked, so a
//! row written under an older rule still reads. One named test per read.
//! Requires a container runtime socket.

use adapter_pg::{PgCommissionStore, PgDatabase, PgFileStore, PgPool, PgWorkflowStore};
use chrono::{SubsecRound, Utc};
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        commission::{
            Commission, CommissionFile, CommissionId, CommissionTitle, FileKey, FileName, NewSlot,
            SlotTitle, SurfaceAddress, SurfaceName, Visibility,
        },
        did::Did,
        handle::Handle,
        user::User,
        workflow::{WorkflowId, WorkflowName},
    },
    ports::{CommissionStore, Database, FileStore, WorkflowStore},
};

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
    let commission = Commission::create(
        title.parse::<CommissionTitle>().expect("title"),
        owner.id.clone(),
        Utc::now(),
        None,
    );
    let id = commission.id;
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions().create(&commission).await.expect("create");
    uow.commit().await.expect("commit");
    id
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
            .all(|summary| summary.visibility == Visibility::Private)
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

// A stored title today's rule refuses still loads, verbatim, through the
// listing and through `find`: loads never re-check a title.
#[tokio::test]
async fn a_stored_title_loads_without_a_recheck() {
    let (pool, _container) = fresh_pool().await;
    let owner = provision(&pool, "did:plc:readslegacy").await;
    let id = seed_commission(&pool, &owner, "Legacy").await;
    sqlx::query("UPDATE commission SET title = '   ' WHERE id = $1")
        .bind(uuid::Uuid::from(id))
        .execute(&pool)
        .await
        .expect("store a title today's rule refuses");
    assert!("   ".parse::<CommissionTitle>().is_err());
    let store = PgCommissionStore::new(pool.clone());

    let listing = store.list_participating(&owner.id).await.expect("list");
    let found = store.find(&id).await.expect("find").expect("present");

    assert_eq!(listing[0].title.as_str(), "   ");
    assert_eq!(found.title.as_str(), "   ");
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
    let name = FileName::try_new("ref-abco.png").expect("filename");
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
    assert!(FileName::try_new("a\u{1}b.png").is_err());

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
    let declared = |commission, address: &SurfaceAddress, title: &str| {
        NewSlot::contributed_at(
            commission,
            address.clone(),
            title.parse::<SlotTitle>().expect("slot title"),
            None,
            owner.id.clone(),
            Utc::now(),
        )
    };
    let slots = [
        declared(mine, &my_address, "Abco"),
        declared(other, &other_address, "Elsewhere"),
        declared(mine, &my_address, "Ember"),
    ];
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions()
        .declare_slots(&slots)
        .await
        .expect("declare");
    uow.commit().await.expect("commit");
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
