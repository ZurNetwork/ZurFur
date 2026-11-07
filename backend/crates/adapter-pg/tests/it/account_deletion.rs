//! The Account delete over PostgreSQL: the shared suite
//! (`test_support::account_deletion_contract`), and the races only row locks decide: a delete
//! waiting on an accept, an accept waiting on a delete, and a delete waiting on a hand-over.
//! Requires a container runtime socket.

use std::sync::Arc;
use std::time::Duration;

use adapter_pg::PgPool;
use domain::elements::{account::AccountId, role::Role};
use test_support::account_deletion_contract::{
    AccountError, DeleteOutcome, Runtime, delete, founded, is_live, offer, recognized, seat,
    try_accept,
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
    assert!(!is_live(&runtime, &account).await, "the Account is deactivated");
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
