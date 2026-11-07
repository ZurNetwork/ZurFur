//! The Account delete over PostgreSQL: the shared suite
//! (`test_support::account_deletion_contract`), and the races only row locks decide: a delete
//! waiting on an accept, an accept waiting on a delete, and a delete waiting on a hand-over.
//! Also the store's own refusals: a hard delete of a deactivated Account deletes nothing, and
//! one racing an accept that took no lock is refused by the facts' foreign key; and what a
//! soft delete keeps. Requires a container runtime socket.

use std::sync::Arc;
use std::time::Duration;

use adapter_pg::PgPool;
use domain::elements::{account::AccountId, role::Role, workflow::WorkflowName};
use test_support::account_deletion_contract::{
    AccountError, DeleteOutcome, Runtime, delete, deleted, founded, is_live, offer, recognized,
    seat, try_accept,
};
use test_support::seeder::CommissionSeeder;

use crate::seed::MintingSeeder;

/// A fresh database's runtime, the commission seeder, and the database kept alive.
async fn fresh() -> (Runtime, Arc<dyn CommissionSeeder>, impl Sized) {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    (runtime, Arc::new(MintingSeeder), pg)
}

test_support::account_deletion_contract_tests!(fresh);

/// Waits, for at most ten seconds, until some connection to this database waits on a lock.
async fn wait_for_a_lock_wait(pool: &PgPool) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let waiting: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM pg_stat_activity \
             WHERE datname = current_database() AND wait_event_type = 'Lock')",
        )
        .fetch_one(pool)
        .await
        .expect("read pg_stat_activity");
        if waiting {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "no connection waited on a lock within ten seconds"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// How many rows of `table` name `account`.
async fn rows_naming(pool: &PgPool, table: &str, account: &AccountId) -> i64 {
    let counting = format!("SELECT count(*) FROM {table} WHERE account_id = $1");
    sqlx::query_scalar(sqlx::AssertSqlSafe(counting))
        .bind(AsRef::<str>::as_ref(account))
        .fetch_one(pool)
        .await
        .expect("count the rows")
}

/// How many of `account`'s invitations are in `state`.
async fn invitations_in(pool: &PgPool, account: &AccountId, state: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM account_invitations WHERE account_id = $1 AND state = $2",
    )
    .bind(AsRef::<str>::as_ref(account))
    .bind(state)
    .fetch_one(pool)
    .await
    .expect("count the invitations")
}

// An accept holding the Account `FOR SHARE` commits its member fact while the delete waits for
// the row: the delete reads the facts after its lock, so it goes soft.
#[tokio::test]
async fn a_delete_waiting_on_an_accept_goes_soft() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    let owner = recognized(&runtime, "did:plc:raceacceptowner").await;
    let invitee = recognized(&runtime, "did:plc:raceacceptinvitee").await;
    let account = founded(&runtime, &owner, "raceaccept.zurfur.app").await;
    offer(&runtime, &account, &owner, &invitee, Role::Member).await;
    let invitation = runtime
        .accounts
        .find_pending_invitation(&account, &invitee)
        .await
        .expect("read the offer")
        .expect("the offer is pending");

    let mut accepting = runtime.database.begin().await.expect("begin the accept");
    accepting
        .accounts()
        .find_for_share(&account)
        .await
        .expect("lock the account")
        .expect("the account is live");
    accepting
        .accounts()
        .accept_invitation(invitation, true)
        .await
        .expect("accept the offer");
    let deleter = runtime.clone();
    let deleting = account.clone();
    let deleting_owner = owner.clone();
    let waiting = tokio::spawn(async move {
        delete(&deleter, &deleting, &deleting_owner)
            .await
            .map(|deleted| deleted.outcome)
    });
    wait_for_a_lock_wait(&runtime.pool).await;
    accepting.commit().await.expect("commit the accept");

    let outcome = waiting.await.expect("the delete finishes");
    assert!(
        matches!(outcome, Ok(DeleteOutcome::Soft)),
        "the delete saw the fact committed while it waited"
    );
    assert!(
        !is_live(&runtime, &account).await,
        "the Account is deactivated"
    );
}

// An accept arriving while a hard delete holds the Account seats no one and records no fact
// once the delete commits.
#[tokio::test]
async fn an_accept_waiting_on_a_delete_answers_not_found_and_seats_no_one() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    let owner = recognized(&runtime, "did:plc:racedeleteowner").await;
    let invitee = recognized(&runtime, "did:plc:racedeleteinvitee").await;
    let account = founded(&runtime, &owner, "racedelete.zurfur.app").await;
    offer(&runtime, &account, &owner, &invitee, Role::Member).await;

    let mut deleting = runtime.database.begin().await.expect("begin the delete");
    deleting
        .accounts()
        .find_for_update(&account)
        .await
        .expect("lock the account")
        .expect("the account is live");
    let facts = deleting
        .accounts()
        .facts(&account)
        .await
        .expect("read the facts");
    assert!(facts.is_empty(), "the Account holds no fact yet");
    deleting
        .accounts()
        .hard_delete(&account)
        .await
        .expect("hard-delete the account");
    let acceptor = runtime.clone();
    let accepting = account.clone();
    let accepting_invitee = invitee.clone();
    let waiting = tokio::spawn(async move {
        try_accept(&acceptor, &accepting, &accepting_invitee)
            .await
            .is_ok()
    });
    wait_for_a_lock_wait(&runtime.pool).await;
    deleting.commit().await.expect("commit the delete");

    let accepted = waiting.await.expect("the accept finishes");
    assert!(!accepted, "the accept is refused");
    assert_eq!(
        rows_naming(&runtime.pool, "account_members", &account).await,
        0,
        "no one is seated"
    );
    assert_eq!(
        rows_naming(&runtime.pool, "account_fact", &account).await,
        0,
        "no fact is recorded"
    );
    assert!(!is_live(&runtime, &account).await, "the Account is gone");
}

// An Owner whose hand-over commits while their delete waits for the Account row is no longer
// its Owner when the delete runs: the delete is refused and the Account stays live.
#[tokio::test]
async fn a_delete_by_an_owner_who_handed_over_meanwhile_is_refused() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    let founder = recognized(&runtime, "did:plc:racehandfounder").await;
    let heir = recognized(&runtime, "did:plc:racehandheir").await;
    let account = founded(&runtime, &founder, "racehand.zurfur.app").await;
    seat(&runtime, &account, &founder, &heir).await;

    let mut handing_over = runtime.database.begin().await.expect("begin the hand-over");
    handing_over
        .accounts()
        .find_for_share(&account)
        .await
        .expect("lock the account")
        .expect("the account is live");
    handing_over
        .accounts()
        .transfer_ownership(&founder, &heir, &account)
        .await
        .expect("hand the account over");
    let deleter = runtime.clone();
    let deleting = account.clone();
    let deleting_founder = founder.clone();
    let waiting = tokio::spawn(async move {
        delete(&deleter, &deleting, &deleting_founder)
            .await
            .map(|deleted| deleted.outcome)
    });
    wait_for_a_lock_wait(&runtime.pool).await;
    handing_over.commit().await.expect("commit the hand-over");

    let outcome = waiting.await.expect("the delete finishes");
    assert!(
        matches!(outcome, Err(AccountError::IncorrectRole)),
        "the former Owner is refused"
    );
    assert!(is_live(&runtime, &account).await, "the Account stays live");
}

// A hard delete aimed at a deactivated Account is refused before it deletes anything: a unit
// that drops the refusal and commits keeps the Owner's seat.
#[tokio::test]
async fn a_refused_hard_delete_deletes_nothing_even_when_committed() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    let owner = recognized(&runtime, "did:plc:refusedhardowner").await;
    let account = founded(&runtime, &owner, "refusedhard.zurfur.app").await;
    let mut deactivating = runtime
        .database
        .begin()
        .await
        .expect("begin the soft delete");
    deactivating
        .accounts()
        .soft_delete(&account)
        .await
        .expect("soft-delete the account");
    deactivating.commit().await.expect("commit the soft delete");

    let mut deleting = runtime
        .database
        .begin()
        .await
        .expect("begin the hard delete");
    let refused = deleting.accounts().hard_delete(&account).await;
    drop(refused);
    deleting.commit().await.expect("commit the refused unit");

    assert_eq!(
        rows_naming(&runtime.pool, "account_members", &account).await,
        1,
        "the Owner is still seated"
    );
}

// An accept that takes no lock on the Account commits its member and fact between the delete's
// facts read and its hard delete: the facts' foreign key refuses the delete, in words naming
// no DID, and nothing is lost.
#[tokio::test]
async fn a_hard_delete_racing_an_unlocked_accept_is_refused_by_the_fact() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    let owner = recognized(&runtime, "did:plc:unlockedowner").await;
    let invitee = recognized(&runtime, "did:plc:unlockedinvitee").await;
    let account = founded(&runtime, &owner, "unlocked.zurfur.app").await;
    offer(&runtime, &account, &owner, &invitee, Role::Member).await;
    let invitation = runtime
        .accounts
        .find_pending_invitation(&account, &invitee)
        .await
        .expect("read the offer")
        .expect("the offer is pending");

    let mut deleting = runtime.database.begin().await.expect("begin the delete");
    deleting
        .accounts()
        .find_for_update(&account)
        .await
        .expect("lock the account")
        .expect("the account is live");
    let facts = deleting
        .accounts()
        .facts(&account)
        .await
        .expect("read the facts");
    assert!(facts.is_empty(), "the Account holds no fact yet");
    let mut accepting = runtime.database.begin().await.expect("begin the accept");
    accepting
        .accounts()
        .accept_invitation(invitation, true)
        .await
        .expect("accept the offer");
    accepting.commit().await.expect("commit the accept");
    let refusal = deleting
        .accounts()
        .hard_delete(&account)
        .await
        .expect_err("the store refuses the stale hard delete")
        .to_string();
    deleting.rollback().await.expect("roll the delete back");

    assert!(refusal.contains("holds a fact"), "{refusal}");
    assert!(
        !refusal.contains("did:"),
        "the refusal names no DID: {refusal}"
    );
    assert_eq!(
        rows_naming(&runtime.pool, "account_members", &account).await,
        2,
        "the member is seated beside the Owner"
    );
    assert_eq!(
        rows_naming(&runtime.pool, "account_fact", &account).await,
        1,
        "the member fact is recorded"
    );
    assert!(is_live(&runtime, &account).await, "the Account stays live");
}

// A soft delete removes no row: members, pending and accepted invitations and boards stay,
// beside the one member fact.
#[tokio::test]
async fn a_soft_delete_keeps_every_row() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    let owner = recognized(&runtime, "did:plc:softkeepowner").await;
    let member = recognized(&runtime, "did:plc:softkeepmember").await;
    let invitee = recognized(&runtime, "did:plc:softkeepinvitee").await;
    let account = founded(&runtime, &owner, "softkeep.zurfur.app").await;
    seat(&runtime, &account, &owner, &member).await;
    offer(&runtime, &account, &owner, &invitee, Role::Member).await;
    let mut boarding = runtime.database.begin().await.expect("begin the board");
    let board_name = "Queue".parse::<WorkflowName>().expect("a board name");
    boarding
        .workflows()
        .create(&board_name, &account)
        .await
        .expect("create the board");
    boarding.commit().await.expect("commit the board");
    let pool = &runtime.pool;
    let members = rows_naming(pool, "account_members", &account).await;
    let pending = invitations_in(pool, &account, "pending").await;
    let accepted = invitations_in(pool, &account, "accepted").await;
    let boards = rows_naming(pool, "workflow", &account).await;
    assert_eq!(
        (members, pending, accepted, boards),
        (2, 1, 1, 1),
        "the seeded rows"
    );

    let outcome = deleted(&runtime, &account, &owner).await;

    assert!(matches!(outcome, DeleteOutcome::Soft), "{outcome}");
    let kept = (
        rows_naming(pool, "account_members", &account).await,
        invitations_in(pool, &account, "pending").await,
        invitations_in(pool, &account, "accepted").await,
        rows_naming(pool, "workflow", &account).await,
    );
    assert_eq!(
        kept,
        (members, pending, accepted, boards),
        "members, invitations and boards"
    );
    assert_eq!(
        rows_naming(pool, "account_fact", &account).await,
        1,
        "one member fact"
    );
}
