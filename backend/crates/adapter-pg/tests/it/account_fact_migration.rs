//! The Account facts table's migration: its backfill seeds made-up Accounts, members and
//! invitations with raw SQL on the schema just before it, then runs it. Checks that each of
//! the backfill's three conditions records the member fact at its time and that an Owner
//! alone with unaccepted offers records none; that an accepted invitation's time out of
//! range stops it by count alone; that a fact is never changed, removed or truncated; that
//! an Account holding one cannot be deleted; and that its kind and time are checked.
//! Requires a container runtime socket (DOCKER_HOST honored).

use adapter_pg::PgPool;
use chrono::{DateTime, Utc};
use sqlx::migrate::MigrateError;
use uuid::Uuid;

use crate::migration_steps::{
    bare_pool, run_migrations_before, run_migrations_through, try_run_migrations_through,
};

/// The facts table's migration, as sqlx numbers it.
pub const FACT_VERSION: i64 = 20261107174548;

/// How the insert-only trigger's refusal begins.
const INSERT_ONLY: &str = "account_fact is insert-only";

/// When the seeded Account's first invitation was accepted.
const FIRST_ACCEPTED: &str = "2026-03-04 05:06:07+00";

/// When the seeded Account's second invitation was accepted.
const LATER_ACCEPTED: &str = "2026-05-06 07:08:09+00";

/// A made-up DID for `label`, padded to did:plc's 24 characters.
pub fn did(label: &str) -> String {
    format!("did:plc:{label:a<24}")
}

/// Seed a User with DID `did`.
pub async fn seed_user(pool: &PgPool, did: &str) {
    sqlx::query(
        "INSERT INTO actor_identity (id, kind, did, state, first_seen)
         VALUES ($1, 'user', $2, 'active', now())",
    )
    .bind(Uuid::now_v7())
    .bind(did)
    .execute(pool)
    .await
    .expect("seed a user's identity");
    sqlx::query("INSERT INTO users (id, created_at) VALUES ($1, now())")
        .bind(did)
        .execute(pool)
        .await
        .expect("seed a user");
}

/// Seed an Account with DID `did`, handle `<label>.zurfur.app`, and no member.
pub async fn seed_account(pool: &PgPool, did: &str, label: &str) {
    sqlx::query(
        "INSERT INTO actor_identity (id, kind, did, state, first_seen)
         VALUES ($1, 'account', $2, 'active', now())",
    )
    .bind(Uuid::now_v7())
    .bind(did)
    .execute(pool)
    .await
    .expect("seed an account's identity");
    sqlx::query(
        "INSERT INTO accounts (id, handle, name, created_at, updated_at)
         VALUES ($1, $2, 'A seeded account', now(), now())",
    )
    .bind(did)
    .bind(format!("{label}.zurfur.app"))
    .execute(pool)
    .await
    .expect("seed an account");
}

/// Seat `user` on `account` at `role`.
async fn seat(pool: &PgPool, account: &str, user: &str, role: &str) {
    sqlx::query("INSERT INTO account_members (account_id, user_id, role) VALUES ($1, $2, $3)")
        .bind(account)
        .bind(user)
        .bind(role)
        .execute(pool)
        .await
        .expect("seat a member");
}

/// Store an invitation of `invitee` into `account` from `inviter`, in `state` since `at`.
async fn offer(pool: &PgPool, account: &str, inviter: &str, invitee: &str, state: &str, at: &str) {
    sqlx::query(
        "INSERT INTO account_invitations
             (id, account_id, invited_user, role, inviter, state, created_at, updated_at)
         VALUES ($1, $2, $3, 'member', $4, $5, $6::text::timestamptz, $6::text::timestamptz)",
    )
    .bind(Uuid::now_v7())
    .bind(account)
    .bind(invitee)
    .bind(inviter)
    .bind(state)
    .bind(at)
    .execute(pool)
    .await
    .expect("store an invitation");
}

/// Every fact row as (account, kind, reached_at), by account.
async fn facts(pool: &PgPool) -> Vec<(String, String, DateTime<Utc>)> {
    sqlx::query_as("SELECT account_id, kind, reached_at FROM account_fact ORDER BY account_id")
        .fetch_all(pool)
        .await
        .expect("read the facts")
}

/// The instant `text` names.
fn instant(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_str(text, "%Y-%m-%d %H:%M:%S%#z")
        .expect("a seeded instant")
        .with_timezone(&Utc)
}

/// The database's own refusal text inside `stopped`.
fn refusal(stopped: &MigrateError) -> String {
    let database_error = match stopped {
        MigrateError::Execute(error) | MigrateError::ExecuteMigration(error, _) => {
            error.as_database_error()
        }
        _ => None,
    };
    database_error
        .map(|error| error.message().to_owned())
        .unwrap_or_else(|| panic!("the database refused the migration: {stopped:?}"))
}

/// The message a refused statement carries.
fn message_of(error: &sqlx::Error) -> String {
    error
        .as_database_error()
        .map(|database_error| database_error.message().to_owned())
        .unwrap_or_else(|| error.to_string())
}

/// A migrated database holding one Account with one fact, answering the Account's DID.
async fn holding_a_fact() -> (PgPool, String, impl Sized) {
    let (pool, db) = test_support::pg::fresh_pool().await;
    let account = did("heldfact");
    seed_account(&pool, &account, "heldfact").await;
    sqlx::query(
        "INSERT INTO account_fact (account_id, kind, reached_at) VALUES ($1, 'member', now())",
    )
    .bind(&account)
    .execute(&pool)
    .await
    .expect("record a fact");
    (pool, account, db)
}

// Seeded before the migration: (i) an Account whose two invitations were accepted and whose
// member then left records the fact at the earlier acceptance; (ii) two Owners and no
// invitation, at the migration's time; (iii) an Owner and a non-Owner member, at the
// migration's time; (iv) a handed-over Account, its heir the one Owner left, at the heir's
// acceptance; and an Owner alone with pending and revoked offers records none.
#[tokio::test]
async fn the_backfill_records_each_account_that_had_a_member() {
    let (pool, _container) = bare_pool().await;
    run_migrations_before(&pool, FACT_VERSION).await;
    let [owner, invitee, other, heir] = ["bfowner", "bfinvitee", "bfother", "bfheir"].map(did);
    for user in [&owner, &invitee, &other, &heir] {
        seed_user(&pool, user).await;
    }
    let [departed, two_owners, with_member, handed_over, alone] = [
        "bfadeparted",
        "bfbtwoowners",
        "bfcwithmember",
        "bfdhandedover",
        "bfealone",
    ]
    .map(did);
    seed_account(&pool, &departed, "departed").await;
    seat(&pool, &departed, &owner, "owner").await;
    offer(
        &pool,
        &departed,
        &owner,
        &invitee,
        "accepted",
        LATER_ACCEPTED,
    )
    .await;
    offer(&pool, &departed, &owner, &other, "accepted", FIRST_ACCEPTED).await;
    seed_account(&pool, &two_owners, "twoowners").await;
    seat(&pool, &two_owners, &owner, "owner").await;
    seat(&pool, &two_owners, &other, "owner").await;
    seed_account(&pool, &with_member, "withmember").await;
    seat(&pool, &with_member, &owner, "owner").await;
    seat(&pool, &with_member, &invitee, "member").await;
    seed_account(&pool, &handed_over, "handedover").await;
    seat(&pool, &handed_over, &heir, "owner").await;
    offer(
        &pool,
        &handed_over,
        &owner,
        &heir,
        "accepted",
        FIRST_ACCEPTED,
    )
    .await;
    seed_account(&pool, &alone, "alone").await;
    seat(&pool, &alone, &owner, "owner").await;
    offer(&pool, &alone, &owner, &invitee, "pending", FIRST_ACCEPTED).await;
    offer(&pool, &alone, &owner, &other, "revoked", LATER_ACCEPTED).await;
    let before = Utc::now();

    run_migrations_through(&pool, FACT_VERSION).await;

    let after = Utc::now();
    let recorded = facts(&pool).await;
    let accounts: Vec<&str> = recorded
        .iter()
        .map(|(account, _, _)| account.as_str())
        .collect();
    assert_eq!(
        accounts,
        [&departed, &two_owners, &with_member, &handed_over].map(String::as_str),
        "every Account that had a member besides its founder, and no other: {recorded:?}"
    );
    assert!(
        recorded.iter().all(|(_, kind, _)| kind == "member"),
        "each fact is the member fact: {recorded:?}"
    );
    let at = |account: &str| {
        recorded
            .iter()
            .find(|(recorded_account, _, _)| recorded_account == account)
            .map(|(_, _, reached_at)| *reached_at)
            .expect("the account has a fact")
    };
    assert_eq!(
        at(&departed),
        instant(FIRST_ACCEPTED),
        "the earliest acceptance"
    );
    assert_eq!(
        at(&handed_over),
        instant(FIRST_ACCEPTED),
        "the heir's acceptance"
    );
    for account in [&two_owners, &with_member] {
        let reached_at = at(account);
        assert!(
            before <= reached_at && reached_at <= after,
            "with no acceptance, the migration's time: {reached_at}"
        );
    }
}

// An accepted invitation whose time is infinite stops the backfill by its count alone,
// naming no DID; the ledger doesn't record it and no fact table exists.
#[tokio::test]
async fn an_acceptance_time_out_of_range_stops_the_backfill_by_count() {
    let (pool, _container) = bare_pool().await;
    run_migrations_before(&pool, FACT_VERSION).await;
    let [owner, invitee] = ["oorowner", "oorinvitee"].map(did);
    seed_user(&pool, &owner).await;
    seed_user(&pool, &invitee).await;
    let account = did("ooraccount");
    seed_account(&pool, &account, "ooraccount").await;
    seat(&pool, &account, &owner, "owner").await;
    offer(&pool, &account, &owner, &invitee, "accepted", "infinity").await;

    let stopped = try_run_migrations_through(&pool, FACT_VERSION)
        .await
        .expect_err("the backfill stops");

    let said = format!("{} | {stopped} | {stopped:?}", refusal(&stopped));
    assert!(
        said.contains("the account fact backfill stops: 1 accepted invitations"),
        "the refusal counts the rows: {said}"
    );
    assert!(!said.contains("did:"), "the refusal names no DID: {said}");
    let table: Option<String> =
        sqlx::query_scalar("SELECT to_regclass('public.account_fact')::text")
            .fetch_one(&pool)
            .await
            .expect("read the catalog");
    assert_eq!(table, None, "no fact table was left behind");
}

// A fact is never updated, deleted or truncated: each is refused by the insert-only
// triggers, and the row is still there.
#[tokio::test]
async fn account_fact_refuses_update_delete_and_truncate() {
    let (pool, account, _db) = holding_a_fact().await;

    for statement in [
        "UPDATE account_fact SET reached_at = now() WHERE account_id = $1",
        "DELETE FROM account_fact WHERE account_id = $1",
    ] {
        let refused = sqlx::query(statement)
            .bind(&account)
            .execute(&pool)
            .await
            .expect_err("the change is refused");
        let said = message_of(&refused);
        assert!(said.starts_with(INSERT_ONLY), "{statement}: {said}");
        assert!(!said.contains("did:"), "{statement} names no DID: {said}");
    }
    let truncated = sqlx::query("TRUNCATE account_fact")
        .execute(&pool)
        .await
        .expect_err("the truncate is refused");
    assert!(
        message_of(&truncated).starts_with(INSERT_ONLY),
        "{truncated}"
    );
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM account_fact WHERE account_id = $1")
        .bind(&account)
        .fetch_one(&pool)
        .await
        .expect("count the facts");
    assert_eq!(left, 1, "the fact is still there");
}

// An Account holding a fact is never deleted from `accounts`: the facts table's foreign
// key refuses it by name.
#[tokio::test]
async fn the_store_refuses_to_delete_an_account_holding_a_fact() {
    let (pool, account, _db) = holding_a_fact().await;

    let refused = sqlx::query("DELETE FROM accounts WHERE id = $1")
        .bind(&account)
        .execute(&pool)
        .await
        .expect_err("the delete is refused");

    let constraint = refused
        .as_database_error()
        .and_then(|database_error| database_error.constraint())
        .map(str::to_owned);
    assert_eq!(
        constraint.as_deref(),
        Some("account_fact_account_id_fkey"),
        "{refused}"
    );
}

// A fact's kind is one the table knows, and its time lies in the years 1 to 9999.
#[tokio::test]
async fn a_fact_s_kind_and_time_are_checked() {
    let (pool, _db) = test_support::pg::fresh_pool().await;
    let account = did("checkedfact");
    seed_account(&pool, &account, "checkedfact").await;
    let refusals = [
        ("commission", "now", "account_fact_kind_known"),
        ("Member", "now", "account_fact_kind_known"),
        ("member", "infinity", "account_fact_reached_at_in_range"),
        (
            "member",
            "10000-01-01 00:00:00+00",
            "account_fact_reached_at_in_range",
        ),
    ];

    for (kind, reached_at, constraint) in refusals {
        let refused = sqlx::query(
            "INSERT INTO account_fact (account_id, kind, reached_at)
             VALUES ($1, $2, $3::text::timestamptz)",
        )
        .bind(&account)
        .bind(kind)
        .bind(reached_at)
        .execute(&pool)
        .await
        .expect_err("the fact is refused");
        let refused_by = refused
            .as_database_error()
            .and_then(|database_error| database_error.constraint())
            .map(str::to_owned);
        assert_eq!(
            refused_by.as_deref(),
            Some(constraint),
            "{kind} at {reached_at}: {refused}"
        );
    }
}
