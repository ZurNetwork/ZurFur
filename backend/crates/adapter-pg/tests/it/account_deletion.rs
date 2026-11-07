//! The Account delete over PostgreSQL: the shared suite
//! (`test_support::account_deletion_contract`), and the races only row locks decide: a delete
//! waiting on an accept, an accept waiting on a delete, and a delete waiting on a hand-over.
//! Also the store's own refusals: a hard delete of a deactivated Account deletes nothing, and
//! one racing an accept that took no lock is refused by the facts' foreign key; and what a
//! soft delete keeps. Then the store's refusal of every write into a deactivated Account,
//! the departure and decline it lets through, and the cascades it never blocks. Requires a
//! container runtime socket.

use std::sync::Arc;
use std::time::Duration;

use adapter_pg::PgPool;
use chrono::Utc;
use domain::elements::{
    account::AccountId,
    commission::CommissionId,
    handle::Handle,
    invitation::Invitation,
    level::Level,
    role::Role,
    user::UserId,
    workflow::{Column, ColumnName, LexOrdering, Workflow, WorkflowName},
};
use domain::ports::AccountDeactivated;
use test_support::account_deletion_contract::{
    AccountError, DeleteOutcome, Runtime, accept, delete, deleted, founded, is_live, offer,
    recognized, seat, try_accept,
};
use test_support::seeder::{CommissionSeeder, seed_commission};

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
// no DID, and nothing is lost. The store's refusal would make that accept wait for the
// delete's lock, so its triggers are disabled here and the foreign key stands alone.
#[tokio::test]
async fn a_hard_delete_racing_an_unlocked_accept_is_refused_by_the_fact() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    for (table, trigger) in [
        (
            "account_invitations",
            "account_invitations_refuse_deactivated",
        ),
        ("account_members", "account_members_refuse_deactivated"),
        ("account_fact", "account_fact_refuse_deactivated"),
    ] {
        let disabling = format!("ALTER TABLE {table} DISABLE TRIGGER {trigger}");
        sqlx::query(sqlx::AssertSqlSafe(disabling))
            .execute(&runtime.pool)
            .await
            .expect("disable the refusal");
    }
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

/// Soft-deletes `account` on a unit of its own, as a delete with a fact does.
async fn deactivate(runtime: &Runtime, account: &AccountId) {
    let mut unit = runtime
        .database
        .begin()
        .await
        .expect("begin the soft delete");
    unit.accounts()
        .soft_delete(account)
        .await
        .expect("soft-delete the account");
    unit.commit().await.expect("commit the soft delete");
}

/// Asserts the store refused `write` into a deactivated Account.
fn assert_refused<T>(refusal: anyhow::Result<T>, write: &str) {
    let Err(err) = refusal else {
        panic!("{write} into a deactivated Account was let through");
    };
    assert!(
        err.downcast_ref::<AccountDeactivated>().is_some(),
        "{write} is refused as deactivated: {err:#}"
    );
}

/// Runs one write on a unit of its own, asserts the store refused it, and rolls back.
macro_rules! assert_unit_refused {
    ($runtime:expr, $write:literal, |$unit:ident| $call:expr) => {{
        let mut $unit = $runtime.database.begin().await.expect("begin the write");
        let refusal = $call.await;
        assert_refused(refusal, $write);
        $unit.rollback().await.expect("roll the refused write back");
    }};
}

/// Asserts a direct insert was refused by the trigger, in words naming no DID.
fn assert_insert_refused(refusal: Result<sqlx::postgres::PgQueryResult, sqlx::Error>, table: &str) {
    let Err(sqlx::Error::Database(db_err)) = refusal else {
        panic!("a direct {table} insert into a deactivated Account was let through");
    };
    assert_eq!(
        db_err.constraint(),
        Some("account_deactivated"),
        "{table}: {db_err}"
    );
    let message = db_err.message();
    assert!(
        !message.contains("did:"),
        "the refusal names no DID: {message}"
    );
}

/// A board on `account` holding one column with one card: a commission `owner` holds.
async fn a_board_with_a_card(
    runtime: &Runtime,
    account: &AccountId,
    owner: &UserId,
) -> (Workflow, Column, CommissionId) {
    let commission = seed_commission(
        &*runtime.database,
        &MintingSeeder,
        owner,
        Level::Private,
        "A made-up commission",
    )
    .await;
    let mut unit = runtime.database.begin().await.expect("begin the board");
    let board_name = "Queue".parse::<WorkflowName>().expect("a board name");
    let mut board = unit
        .workflows()
        .create(&board_name, account)
        .await
        .expect("create the board");
    let column_name = "Open".parse::<ColumnName>().expect("a column name");
    let mut column = board.new_column(column_name, board.visibility);
    column.push(commission).expect("the column takes the card");
    board.insert(0, column.clone()).expect("the board is empty");
    unit.workflows()
        .set_indexes(&board)
        .await
        .expect("persist the column");
    unit.columns()
        .set_commissions(&column)
        .await
        .expect("place the card");
    unit.commit().await.expect("commit the board");
    (board, column, commission)
}

/// How many cards of `commission` sit on any board.
async fn cards_of(pool: &PgPool, commission: &CommissionId) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM workflow_column_commission WHERE commission_id = $1")
        .bind(uuid::Uuid::from(*commission))
        .fetch_one(pool)
        .await
        .expect("count the cards")
}

// Each write into a deactivated Account, called without the use case's lock, is refused by
// the store inside its statement: memberships, invitations, the transfer, handle changes,
// facts and boards alike.
#[tokio::test]
async fn the_store_refuses_every_write_into_a_deactivated_account() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    let owner = recognized(&runtime, "did:plc:refuseowner").await;
    let member = recognized(&runtime, "did:plc:refusemember").await;
    let invitee = recognized(&runtime, "did:plc:refuseinvitee").await;
    let stranger = recognized(&runtime, "did:plc:refusestranger").await;
    let account = founded(&runtime, &owner, "refuse.zurfur.app").await;
    seat(&runtime, &account, &owner, &member).await;
    offer(&runtime, &account, &owner, &invitee, Role::Member).await;
    let (board, column, _) = a_board_with_a_card(&runtime, &account, &owner).await;
    let pending = runtime
        .accounts
        .find_pending_invitation(&account, &invitee)
        .await
        .expect("read the offer")
        .expect("the offer is pending");
    deactivate(&runtime, &account).await;
    let fresh_offer = Invitation::issue(
        account.clone(),
        stranger.clone(),
        Role::Member,
        owner.clone(),
        Utc::now(),
    );
    let second_board = "Later".parse::<WorkflowName>().expect("a board name");
    let mut renamed = column.clone();
    renamed.name = "Done".parse::<ColumnName>().expect("a column name");

    assert_unit_refused!(runtime, "a role grant", |unit| unit.accounts().grant_role(
        &member,
        &account,
        &Role::Admin
    ));
    assert_unit_refused!(runtime, "an invitation issue", |unit| unit
        .accounts()
        .create_invitation(&fresh_offer));
    assert_unit_refused!(runtime, "an accept", |unit| unit
        .accounts()
        .accept_invitation(pending, true));
    assert_unit_refused!(runtime, "a transfer", |unit| unit
        .accounts()
        .transfer_ownership(&owner, &member, &account));
    assert_unit_refused!(runtime, "a board insert", |unit| unit
        .workflows()
        .create(&second_board, &account));
    assert_unit_refused!(runtime, "a board delete", |unit| unit
        .workflows()
        .delete(&board.id));
    assert_unit_refused!(runtime, "a column upsert", |unit| unit
        .workflows()
        .set_indexes(&board));
    assert_unit_refused!(runtime, "a card placement", |unit| unit
        .columns()
        .set_commissions(&column));
    assert_unit_refused!(runtime, "a column rename", |unit| unit
        .columns()
        .rename(&renamed));

    let pool = &runtime.pool;
    let fact = sqlx::query(
        "INSERT INTO account_fact (account_id, kind, reached_at) VALUES ($1, 'member', now())",
    )
    .bind(AsRef::<str>::as_ref(&account))
    .execute(pool)
    .await;
    assert_insert_refused(fact, "account_fact");
    let membership = sqlx::query(
        "INSERT INTO account_members (account_id, user_id, role, listed_on_profile) \
         VALUES ($1, $2, 'member', true)",
    )
    .bind(AsRef::<str>::as_ref(&account))
    .bind(stranger.as_ref())
    .execute(pool)
    .await;
    assert_insert_refused(membership, "account_members");
    let handle_change = sqlx::query(
        "INSERT INTO account_handle_changes (id, account_id, old_handle, new_handle, changed_at) \
         VALUES ($1, $2, 'refuse.zurfur.app', 'other.zurfur.app', now())",
    )
    .bind(uuid::Uuid::now_v7())
    .bind(AsRef::<str>::as_ref(&account))
    .execute(pool)
    .await;
    assert_insert_refused(handle_change, "account_handle_changes");
}

// A member leaving a deactivated Account passes the store: their membership goes, their
// invitee is re-homed onto their own parent, and their pending offer is revoked.
#[tokio::test]
async fn a_members_departure_from_a_deactivated_account_passes() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    let owner = recognized(&runtime, "did:plc:departowner").await;
    let member = recognized(&runtime, "did:plc:departmember").await;
    let child = recognized(&runtime, "did:plc:departchild").await;
    let invitee = recognized(&runtime, "did:plc:departinvitee").await;
    let account = founded(&runtime, &owner, "depart.zurfur.app").await;
    seat(&runtime, &account, &owner, &member).await;
    let child_offer = Invitation::issue(
        account.clone(),
        child.clone(),
        Role::Member,
        member.clone(),
        Utc::now(),
    );
    let invitee_offer = Invitation::issue(
        account.clone(),
        invitee.clone(),
        Role::Member,
        member.clone(),
        Utc::now(),
    );
    let mut issuing = runtime.database.begin().await.expect("begin the offers");
    issuing
        .accounts()
        .create_invitation(&child_offer)
        .await
        .expect("the member offers the child a seat");
    issuing
        .accounts()
        .create_invitation(&invitee_offer)
        .await
        .expect("the member offers the invitee a seat");
    issuing.commit().await.expect("commit the offers");
    accept(&runtime, &account, &child).await;
    deactivate(&runtime, &account).await;

    let mut leaving = runtime.database.begin().await.expect("begin the departure");
    leaving
        .accounts()
        .leave(&member, &account)
        .await
        .expect("the store lets the departure through");
    leaving.commit().await.expect("commit the departure");

    let pool = &runtime.pool;
    let child_parent: Option<String> = sqlx::query_scalar(
        "SELECT parent FROM account_members WHERE account_id = $1 AND user_id = $2",
    )
    .bind(AsRef::<str>::as_ref(&account))
    .bind(child.as_ref())
    .fetch_one(pool)
    .await
    .expect("read the child's parent");
    assert_eq!(
        child_parent.as_deref(),
        Some(owner.as_ref()),
        "the child is re-homed onto the Owner"
    );
    assert_eq!(
        rows_naming(pool, "account_members", &account).await,
        2,
        "the Owner and the child stay; the member is gone"
    );
    assert_eq!(
        invitations_in(pool, &account, "pending").await,
        0,
        "the member's pending offer is revoked"
    );
    assert_eq!(
        invitations_in(pool, &account, "revoked").await,
        1,
        "one revoked offer"
    );
}

// An invitee declining a deactivated Account's pending offer passes the store.
#[tokio::test]
async fn a_decline_of_a_deactivated_accounts_offer_passes() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    let owner = recognized(&runtime, "did:plc:declineowner").await;
    let invitee = recognized(&runtime, "did:plc:declineinvitee").await;
    let account = founded(&runtime, &owner, "decline.zurfur.app").await;
    offer(&runtime, &account, &owner, &invitee, Role::Member).await;
    let pending = runtime
        .accounts
        .find_pending_invitation(&account, &invitee)
        .await
        .expect("read the offer")
        .expect("the offer is pending");
    deactivate(&runtime, &account).await;

    let mut declining = runtime.database.begin().await.expect("begin the decline");
    declining
        .accounts()
        .revoke_invitation(&pending.id)
        .await
        .expect("the store lets the decline through");
    declining.commit().await.expect("commit the decline");

    let pool = &runtime.pool;
    assert_eq!(
        invitations_in(pool, &account, "pending").await,
        0,
        "no offer is pending"
    );
    assert_eq!(
        invitations_in(pool, &account, "revoked").await,
        1,
        "the offer is declined"
    );
}

// An empty Account's hard delete cascades into its boards, columns, cards and handle-change
// rows after its own row is gone: the refusal lets every cascaded delete through.
#[tokio::test]
async fn a_hard_delete_cascades_through_the_refusal() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    let owner = recognized(&runtime, "did:plc:cascadeowner").await;
    let account = founded(&runtime, &owner, "cascade.zurfur.app").await;
    let (board, _, commission) = a_board_with_a_card(&runtime, &account, &owner).await;
    let old = "cascade.zurfur.app".parse::<Handle>().expect("a handle");
    let new = "cascaded.zurfur.app".parse::<Handle>().expect("a handle");
    let mut renaming = runtime.database.begin().await.expect("begin the change");
    renaming
        .accounts()
        .change_handle(&account, &old, &new, Utc::now())
        .await
        .expect("change the handle");
    renaming.commit().await.expect("commit the change");

    let outcome = deleted(&runtime, &account, &owner).await;

    assert!(matches!(outcome, DeleteOutcome::Hard), "{outcome}");
    let pool = &runtime.pool;
    let columns: i64 =
        sqlx::query_scalar("SELECT count(*) FROM workflow_column WHERE workflow_id = $1")
            .bind(uuid::Uuid::from(board.id))
            .fetch_one(pool)
            .await
            .expect("count the columns");
    let left = (
        rows_naming(pool, "workflow", &account).await,
        columns,
        cards_of(pool, &commission).await,
        rows_naming(pool, "account_handle_changes", &account).await,
    );
    assert_eq!(
        left,
        (0, 0, 0, 0),
        "boards, columns, cards and handle changes"
    );
}

// A User deleting their own commission takes its card off a deactivated Account's board:
// card deletes are never refused.
#[tokio::test]
async fn a_commission_delete_cascades_through_a_deactivated_board() {
    let pg = test_support::pg_app::pg_runtime().await;
    let runtime = pg.runtime.clone();
    let owner = recognized(&runtime, "did:plc:boardcascadeowner").await;
    let account = founded(&runtime, &owner, "boardcascade.zurfur.app").await;
    let (_, _, commission) = a_board_with_a_card(&runtime, &account, &owner).await;
    deactivate(&runtime, &account).await;

    let mut deleting = runtime.database.begin().await.expect("begin the delete");
    deleting
        .commissions()
        .delete(&commission)
        .await
        .expect("delete the commission");
    deleting.commit().await.expect("commit the delete");

    assert_eq!(
        cards_of(&runtime.pool, &commission).await,
        0,
        "the card left the board"
    );
}
