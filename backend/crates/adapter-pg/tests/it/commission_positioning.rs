//! Commission positioning over PostgreSQL, against a throwaway container:
//! the board rail (**placement is a
//! card on a board** — "placement = workflow membership rows,
//! account-side") and the view-grant key's upsert/hard-delete. The writes go
//! through the [`UnitOfWork`]'s views; the reads through the pool-backed stores.
//! Requires a container runtime socket.

use adapter_pg::{PgColumnStore, PgCommissionStore, PgDatabase, PgPool, PgWorkflowStore};
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountId, AccountName},
        commission::{Commission, CommissionTitle, GrantLevel},
        did::Did,
        handle::Handle,
        user::User,
        workflow::{ColumnId, ColumnName, LexOrdering, WorkflowId, WorkflowName},
    },
    ports::{ColumnStore, CommissionStore, Database, WorkflowStore},
};

/// A fresh, fully migrated private database — a clone of the shared template
/// (see `test_support::pg`). The second element keeps the shared container
/// alive for the test's duration.
async fn fresh_pool() -> (PgPool, impl Sized) {
    test_support::pg::fresh_pool().await
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
        "A ref sheet".parse::<CommissionTitle>().expect("title"),
        owner.id.clone(),
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
        owner.id.clone(),
        Did::new(format!("did:plc:acct-{handle}")),
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

/// Seed a committed board with one column for `account`, returning both ids.
async fn seed_board(pool: &PgPool, account: &AccountId, name: &str) -> (WorkflowId, ColumnId) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let board_name = name.parse::<WorkflowName>().expect("board name");
    let mut workflow = uow
        .workflows()
        .create(&board_name, account)
        .await
        .expect("create the board");
    let column_name = "Open".parse::<ColumnName>().expect("column name");
    let column = workflow.new_column(column_name, workflow.visibility.clone());
    let column_id = column.id.clone();
    workflow.insert(0, column).expect("the board is empty");
    uow.workflows()
        .set_indexes(&workflow)
        .await
        .expect("persist the column");
    uow.commit().await.expect("commit");
    (workflow.id, column_id)
}

/// Put `commission` at the end of `column`, committed.
async fn place_card(
    pool: &PgPool,
    column_id: &ColumnId,
    commission: domain::elements::commission::CommissionId,
) {
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    let mut column = PgColumnStore::new(pool.clone())
        .find(column_id)
        .await
        .expect("read the column")
        .expect("the column exists");
    column.push(commission).expect("the column takes the card");
    uow.columns()
        .set_commissions(&column)
        .await
        .expect("place the card");
    uow.commit().await.expect("commit");
}

// AC1/AC2 (pg) — **placement is a card on a board**. Positioning puts the
// commission in a column; the same commission sits on N accounts' boards at
// once with no conflict, because no account ever claims it (DD `29130754` D1:
// users own commissions, accounts own positioning).
//
// The append-only placement log AC3 cached has no referent any more: it was
// 1:1-current, which is exactly the managing-account model this DD superseded.
#[tokio::test]
async fn a_commission_sits_on_many_accounts_boards_at_once() {
    let (pool, _c) = fresh_pool().await;
    let (_owner, id) = seed_commission(&pool, "did:plc:place-owner").await;
    let a = seed_account(&pool, "did:plc:acc-a", "posa.example.com").await;
    let b = seed_account(&pool, "did:plc:acc-b", "posb.example.com").await;
    let (board_a, column_a) = seed_board(&pool, &a, "Board A").await;
    let (board_b, column_b) = seed_board(&pool, &b, "Board B").await;

    let store = PgCommissionStore::new(pool.clone());

    assert!(
        store
            .current_column_of_workflow(&id, &board_a)
            .await
            .unwrap()
            .is_none(),
        "an unpositioned commission sits on no board (still valid — AC6)"
    );

    for (column, board) in [(&column_a, &board_a), (&column_b, &board_b)] {
        place_card(&pool, column, id).await;

        assert_eq!(
            store
                .current_column_of_workflow(&id, board)
                .await
                .unwrap()
                .map(|found| found.id),
            Some(column.clone()),
            "the board that positioned it holds the card",
        );
        assert_eq!(
            store.current_position_in_column(&id, column).await.unwrap(),
            Some(0),
            "at the index the domain put it",
        );
    }

    // Both boards hold it — the NxM intent, native.
    assert!(
        store
            .current_column_of_workflow(&id, &board_a)
            .await
            .unwrap()
            .is_some(),
        "the first board did not lose the card to the second",
    );

    // The board is the account's: deleting it takes the card, not the commission.
    {
        let db = PgDatabase::new(pool.clone());
        let mut uow = db.begin().await.expect("begin");
        uow.workflows()
            .delete(&board_a)
            .await
            .expect("delete the board");
        uow.commit().await.expect("commit");
    }
    assert!(
        PgWorkflowStore::new(pool.clone())
            .find(&board_a)
            .await
            .unwrap()
            .is_none(),
        "the board is gone",
    );
    assert!(
        store.find(&id).await.unwrap().is_some(),
        "the User-owned commission survives its board",
    );
    assert!(
        store
            .current_column_of_workflow(&id, &board_b)
            .await
            .unwrap()
            .is_some(),
        "and the other account's board still positions it",
    );
}

// AC4 (pg) — a view grant upserts (re-granting replaces the level) and revoking
// hard-deletes it (view_grant answers None immediately); a repeat revoke is a
// no-op answering false.
#[tokio::test]
async fn view_grant_upserts_and_revoke_hard_deletes() {
    let (pool, _c) = fresh_pool().await;
    let (_owner, id) = seed_commission(&pool, "did:plc:grant-owner").await;
    // A view grant is issued to a **User**, never an Account (DD `29130754` D3,
    // amended 2026-09-04) — both halves of the port now say so.
    let grantee = provision(&pool, "did:plc:grant-holder").await.id;

    let db = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());

    // Grant Presentation, then re-grant Total — the key replaces, not stacks.
    for level in [GrantLevel::Presentation, GrantLevel::Total] {
        let mut uow = db.begin().await.expect("begin");
        uow.commissions()
            .grant_view(&id, &grantee, level)
            .await
            .expect("grant");
        uow.commit().await.expect("commit");
    }
    assert_eq!(
        store.view_grant(&id, &grantee).await.unwrap(),
        Some(GrantLevel::Total),
        "re-granting replaces the level (one key per grantee, upsert)",
    );

    // Revoke — the key is gone immediately.
    let mut uow = db.begin().await.expect("begin");
    let removed = uow
        .commissions()
        .revoke_view(&id, &grantee)
        .await
        .expect("revoke");
    uow.commit().await.expect("commit");
    assert!(
        removed,
        "revoking an existing key reports a real transition"
    );
    assert!(
        store.view_grant(&id, &grantee).await.unwrap().is_none(),
        "a revoked key hard-deletes — its row is gone (DD D5)",
    );

    // Revoking again is a no-op answering false (the no-duplicate-entry key).
    let mut uow = db.begin().await.expect("begin");
    let removed = uow
        .commissions()
        .revoke_view(&id, &grantee)
        .await
        .expect("revoke again");
    uow.commit().await.expect("commit");
    assert!(
        !removed,
        "revoking a non-existent key is a no-op answering false"
    );
}

// A dropped unit of work rolls back a card and a grant together — the pg drop =
// rollback guarantee (DD 24150017) holds for the board tables too.
#[tokio::test]
async fn a_dropped_unit_rolls_back_the_card_and_the_grant() {
    let (pool, _c) = fresh_pool().await;
    let (_owner, id) = seed_commission(&pool, "did:plc:rollback-owner").await;
    let account = seed_account(&pool, "did:plc:acc-r", "posr.example.com").await;
    let (board, column_id) = seed_board(&pool, &account, "Rollback Board").await;
    let grantee = provision(&pool, "did:plc:rollback-holder").await.id;

    let db = PgDatabase::new(pool.clone());
    let store = PgCommissionStore::new(pool.clone());

    {
        let mut uow = db.begin().await.expect("begin");
        let mut column = PgColumnStore::new(pool.clone())
            .find(&column_id)
            .await
            .expect("read the column")
            .expect("the column exists");
        column.push(id).expect("the column takes the card");
        uow.columns()
            .set_commissions(&column)
            .await
            .expect("place the card");
        uow.commissions()
            .grant_view(&id, &grantee, GrantLevel::Total)
            .await
            .expect("grant");
        // Drop without commit → both writes are discarded.
    }

    assert!(
        store
            .current_column_of_workflow(&id, &board)
            .await
            .unwrap()
            .is_none(),
        "a dropped unit puts no card on the board",
    );
    assert!(
        store.view_grant(&id, &grantee).await.unwrap().is_none(),
        "a dropped unit persists no grant",
    );
}
