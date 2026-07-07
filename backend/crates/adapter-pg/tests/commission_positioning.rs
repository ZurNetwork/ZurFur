//! Commission positioning over PostgreSQL (ZMVP-70; Ownership Separation DD
//! `29130754`), against a throwaway container: the append-only placement log and
//! its cached current pointer, and the view-grant key's upsert/hard-delete. The
//! writes go through the [`UnitOfWork`]'s commissions view; the reads through the
//! pool-backed [`PgCommissionStore`]. Requires a container runtime socket.

use adapter_pg::{PgCommissionStore, PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        commission::{Commission, CommissionTitle, GrantLevel},
        did::Did,
        handle::Handle,
        user::User,
    },
    ports::{CommissionStore, Database},
};
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};

async fn fresh_pool() -> (PgPool, impl Sized) {
    let container = Postgres::default()
        .start()
        .await
        .expect("postgres container should start");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped postgres port");
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");
    let pool = adapter_pg::connect(&database_url)
        .await
        .expect("pool connects");
    adapter_pg::migrate(&pool).await.expect("migrations run");
    (pool, container)
}

async fn provision(pool: &PgPool, did: &str) -> User {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let user = uow
        .users()
        .provision(&Did::new(did.to_string()))
        .await
        .expect("provision");
    uow.commit().await.expect("commit");
    user
}

/// Seed a committed commission owned by a freshly provisioned user; returns both.
async fn seed_commission(
    pool: &PgPool,
    owner_did: &str,
) -> (User, domain::elements::commission::CommissionId) {
    let owner = provision(pool, owner_did).await;
    let commission = Commission::create(
        CommissionTitle::try_new("A ref sheet").expect("title"),
        owner.id,
        Utc::now(),
        None,
    );
    let id = commission.id;
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.commissions().create(&commission).await.expect("create");
    uow.commit().await.expect("commit");
    (owner, id)
}

/// Seed a committed account (its owner is provisioned first — `account_members`
/// references `users`), returning its id.
async fn seed_account(pool: &PgPool, owner_did: &str, handle: &str) -> AccountId {
    let owner = provision(pool, owner_did).await;
    let (account, membership) = Account::open(
        owner.id,
        Did::new(format!("did:plc:acct-{handle}")),
        Handle::try_new(handle).expect("handle"),
        AccountName::try_new("PG Studio").expect("name"),
        Utc::now(),
    );
    let id = account.id;
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts()
        .create(&account, &membership)
        .await
        .expect("found account");
    uow.commit().await.expect("commit");
    id
}

// AC1/AC2/AC3 (pg) — placement appends an ever-growing log the current-placement
// pointer tracks: after every (re)placement the cache equals the latest log row,
// current = greatest seq, origin = least.
#[tokio::test]
async fn placement_log_appends_and_current_pointer_tracks_latest() {
    let (pool, _c) = fresh_pool().await;
    let (owner, id) = seed_commission(&pool, "did:plc:place-owner").await;
    let a = seed_account(&pool, "did:plc:acc-a", "posa.example.com").await;
    let b = seed_account(&pool, "did:plc:acc-b", "posb.example.com").await;

    let db = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());

    assert!(
        store.current_placement(id).await.unwrap().is_none(),
        "an unplaced commission has no current placement (still valid — AC6)"
    );

    for account in [a, b, a] {
        let mut uow = db.begin().await.expect("begin");
        uow.commissions()
            .place(id, account, owner.id, Utc::now())
            .await
            .expect("place");
        uow.commit().await.expect("commit");

        let log = store.placement_log(id).await.unwrap();
        let latest = log.last().unwrap();
        let current = store.current_placement(id).await.unwrap().expect("current");
        assert_eq!(
            (current.seq, current.account_id, current.placed_by),
            (latest.seq, latest.account_id, latest.placed_by),
            "the cached current pointer equals the latest log row",
        );
        assert_eq!(current.account_id, account, "current = just-placed account");
    }

    let log = store.placement_log(id).await.unwrap();
    assert_eq!(
        log.len(),
        3,
        "the log is append-only — three placements, three rows"
    );
    assert_eq!(log.first().unwrap().account_id, a, "origin = first row");
    assert_eq!(log.last().unwrap().account_id, a, "current = latest row");
    assert!(
        log[0].seq < log[1].seq && log[1].seq < log[2].seq,
        "seq orders the log"
    );
    assert_eq!(
        log[0].placed_by, owner.id,
        "the placement records who placed it"
    );
}

// AC4 (pg) — a view grant upserts (re-granting replaces the level) and revoking
// hard-deletes it (view_grant answers None immediately); a repeat revoke is a
// no-op answering false.
#[tokio::test]
async fn view_grant_upserts_and_revoke_hard_deletes() {
    let (pool, _c) = fresh_pool().await;
    let (_owner, id) = seed_commission(&pool, "did:plc:grant-owner").await;
    let account = seed_account(&pool, "did:plc:acc-g", "posg.example.com").await;

    let db = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());

    // Grant Presentation, then re-grant Total — the key replaces, not stacks.
    for level in [GrantLevel::Presentation, GrantLevel::Total] {
        let mut uow = db.begin().await.expect("begin");
        uow.commissions()
            .grant_view(id, account, level)
            .await
            .expect("grant");
        uow.commit().await.expect("commit");
    }
    assert_eq!(
        store.view_grant(id, account).await.unwrap(),
        Some(GrantLevel::Total),
        "re-granting replaces the level (one key per account, upsert)",
    );

    // Revoke — the key is gone immediately.
    let mut uow = db.begin().await.expect("begin");
    let removed = uow
        .commissions()
        .revoke_view(id, account)
        .await
        .expect("revoke");
    uow.commit().await.expect("commit");
    assert!(
        removed,
        "revoking an existing key reports a real transition"
    );
    assert!(
        store.view_grant(id, account).await.unwrap().is_none(),
        "a revoked key hard-deletes — its row is gone (DD D5)",
    );

    // Revoking again is a no-op answering false (the no-duplicate-entry key).
    let mut uow = db.begin().await.expect("begin");
    let removed = uow
        .commissions()
        .revoke_view(id, account)
        .await
        .expect("revoke again");
    uow.commit().await.expect("commit");
    assert!(
        !removed,
        "revoking a non-existent key is a no-op answering false"
    );
}

// A dropped unit of work rolls back placement and grant writes together — the
// pg drop = rollback guarantee (DD 24150017) holds for the new tables too.
#[tokio::test]
async fn a_dropped_unit_rolls_back_placement_and_grant() {
    let (pool, _c) = fresh_pool().await;
    let (owner, id) = seed_commission(&pool, "did:plc:rollback-owner").await;
    let account = seed_account(&pool, "did:plc:acc-r", "posr.example.com").await;

    let db = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());

    {
        let mut uow = db.begin().await.expect("begin");
        uow.commissions()
            .place(id, account, owner.id, Utc::now())
            .await
            .expect("place");
        uow.commissions()
            .grant_view(id, account, GrantLevel::Total)
            .await
            .expect("grant");
        // Drop without commit → both writes are discarded.
    }

    assert!(
        store.current_placement(id).await.unwrap().is_none(),
        "a dropped unit persists no placement",
    );
    assert!(
        store.placement_log(id).await.unwrap().is_empty(),
        "a dropped unit appends no placement-log row",
    );
    assert!(
        store.view_grant(id, account).await.unwrap().is_none(),
        "a dropped unit persists no grant",
    );
}
