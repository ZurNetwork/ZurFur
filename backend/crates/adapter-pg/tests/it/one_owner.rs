//! The one-Owner index's migration: tests seed made-up Accounts and members with raw SQL.
//! Checks that the index refuses a second Owner, by an insert and by an update, and keeps
//! every other role in any number; that two Accounts holding two Owners each stop the migration
//! naming the count alone, never a DID, unrecorded, repaired by nothing and with no index added;
//! and that an ownership transfer through the store passes the index and leaves one Owner.
//! Requires a container runtime socket (DOCKER_HOST honored).

use adapter_pg::{PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountName},
        did::Did,
        handle::Handle,
    },
    ports::Database,
};
use sqlx::migrate::MigrateError;

use crate::account_fact_migration::{did, seed_account, seed_user};
use crate::migration_steps::{
    run_migrations_before, run_migrations_through, try_run_migrations_through,
};

/// The one-Owner migration, as sqlx numbers it.
const OWNER_VERSION: i64 = 20261107174551;

/// The index that holds an Account to one Owner.
const OWNER_INDEX: &str = "account_members_one_owner";

/// How the migration's refusal reads for two Accounts, which names no Account or User.
const TWO_ACCOUNTS_REFUSED: &str = "stops: 2 accounts have more than one owner";

/// Seat `user` on `account` at `role`.
async fn seat(pool: &PgPool, account: &str, user: &str, role: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO account_members (account_id, user_id, role) VALUES ($1, $2, $3)")
        .bind(account)
        .bind(user)
        .bind(role)
        .execute(pool)
        .await
        .map(drop)
}

/// Seeds the Account `label` and each of `users`, the first seated as its Owner and the others
/// as Members, and answers the Account's DID.
async fn seed_account_with_members(pool: &PgPool, label: &str, users: &[String]) -> String {
    let account = did(label);
    seed_account(pool, &account, label).await;
    for (place, user) in users.iter().enumerate() {
        seed_user(pool, user).await;
        let role = if place == 0 { "owner" } else { "member" };
        seat(pool, &account, user, role)
            .await
            .expect("seat a member");
    }
    account
}

/// The constraint or index that refused `error`, if one did.
fn refusing_constraint(error: &sqlx::Error) -> Option<String> {
    error
        .as_database_error()
        .and_then(|database_error| database_error.constraint())
        .map(str::to_owned)
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

/// Whether the ledger records the one-Owner migration.
async fn ledger_records_it(pool: &PgPool) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = $1)")
        .bind(OWNER_VERSION)
        .fetch_one(pool)
        .await
        .expect("read the ledger")
}

/// Whether the one-Owner index exists.
async fn index_exists(pool: &PgPool) -> bool {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_indexes WHERE indexname = $1)")
        .bind(OWNER_INDEX)
        .fetch_one(pool)
        .await
        .expect("read the catalog's indexes")
}

/// How many Owners `account` holds.
async fn owners_of(pool: &PgPool, account: &str) -> i64 {
    sqlx::query_scalar(
        "SELECT count(*) FROM account_members WHERE account_id = $1 AND role = 'owner'",
    )
    .bind(account)
    .fetch_one(pool)
    .await
    .expect("count the owners")
}

// On the migrated schema, a second Owner is refused by the index, whether inserted or made by
// promoting a member; the refused write leaves one Owner, and Admins and Members come in any
// number, and another Account holds its own Owner.
#[tokio::test]
async fn a_second_owner_is_refused_by_the_store() {
    let (pool, _db) = test_support::pg::fresh_pool().await;
    let users = [did("firstowner"), did("firstmember"), did("secondmember")];
    let account = seed_account_with_members(&pool, "ownedaccount", &users).await;
    let other_users = [did("otherowner")];
    let other = seed_account_with_members(&pool, "otheraccount", &other_users).await;
    let newcomer = did("newcomer");
    seed_user(&pool, &newcomer).await;

    let inserted = seat(&pool, &account, &newcomer, "owner")
        .await
        .expect_err("the index refuses a second Owner");
    assert_eq!(
        refusing_constraint(&inserted).as_deref(),
        Some(OWNER_INDEX),
        "an insert: {inserted}"
    );

    let promoting =
        "UPDATE account_members SET role = 'owner' WHERE account_id = $1 AND user_id = $2";
    let promoted = sqlx::query(promoting)
        .bind(&account)
        .bind(&users[2])
        .execute(&pool)
        .await
        .expect_err("the index refuses a promotion to a second Owner");
    assert_eq!(
        refusing_constraint(&promoted).as_deref(),
        Some(OWNER_INDEX),
        "an update: {promoted}"
    );

    assert_eq!(owners_of(&pool, &account).await, 1, "one Owner is left");
    assert_eq!(
        owners_of(&pool, &other).await,
        1,
        "another Account keeps its own"
    );
    seat(&pool, &account, &newcomer, "admin")
        .await
        .expect("an Admin seats beside the Owner");
}

// Two Accounts planted with two Owners each, before the migration, stop it naming the count of
// two and no DID; it stays unrecorded, repairs no row and adds no index. With each double
// Owner demoted, a clean run is recorded and adds the index.
#[tokio::test]
async fn the_migration_refuses_two_owners_by_count_without_a_did() {
    let db = test_support::pg::bare_db().await;
    let mut pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    run_migrations_before(&pool, OWNER_VERSION).await;
    let first_users = [did("firstowner"), did("secondowner")];
    let first = seed_account_with_members(&pool, "firstaccount", &first_users).await;
    let second_users = [did("thirdowner"), did("fourthowner")];
    let second = seed_account_with_members(&pool, "secondaccount", &second_users).await;
    let single_users = [did("soleowner")];
    seed_account_with_members(&pool, "singleaccount", &single_users).await;
    let doubled = [(&first, &first_users[1]), (&second, &second_users[1])];
    for (account, user) in doubled {
        sqlx::query(
            "UPDATE account_members SET role = 'owner' WHERE account_id = $1 AND user_id = $2",
        )
        .bind(account)
        .bind(user)
        .execute(&pool)
        .await
        .expect("plant a second Owner before the index exists");
    }

    let stopped = try_run_migrations_through(&pool, OWNER_VERSION)
        .await
        .expect_err("the migration stops");

    let said = format!("{} | {stopped} | {stopped:?}", refusal(&stopped));
    assert!(
        said.contains(TWO_ACCOUNTS_REFUSED),
        "names the count: {said}"
    );
    assert!(!said.contains("did:"), "names no DID: {said}");
    assert!(
        !ledger_records_it(&pool).await,
        "the ledger does not record the stopped migration"
    );
    assert!(!index_exists(&pool).await, "the index was not added");
    assert_eq!(
        owners_of(&pool, &first).await,
        2,
        "the first Account is not repaired"
    );
    assert_eq!(
        owners_of(&pool, &second).await,
        2,
        "the second Account is not repaired"
    );

    pool.close().await;
    pool = adapter_pg::connect(db.url())
        .await
        .expect("pool reconnects");
    for (account, user) in doubled {
        sqlx::query(
            "UPDATE account_members SET role = 'admin' WHERE account_id = $1 AND user_id = $2",
        )
        .bind(account)
        .bind(user)
        .execute(&pool)
        .await
        .expect("demote the second Owner");
    }
    run_migrations_through(&pool, OWNER_VERSION).await;
    assert!(ledger_records_it(&pool).await, "the clean run is recorded");
    assert!(index_exists(&pool).await, "the clean run adds the index");
}

// An ownership transfer through the store, with the index in place, demotes the outgoing Owner
// before it promotes the heir, so it commits and one Owner is left: the heir.
#[tokio::test]
async fn a_transfer_passes_the_one_owner_index() {
    let (pool, _db) = test_support::pg::fresh_pool().await;
    assert!(index_exists(&pool).await, "the index is in place");
    let database = PgDatabase::new(pool.clone());
    let owner_did = Did::from(did("transferowner"));
    let heir_did = Did::from(did("transferheir"));
    let account_did = Did::from(did("transferaccount"));
    let handle = "transfer-account.example.com"
        .parse::<Handle>()
        .expect("a handle");
    let name = "Hand-Off".parse::<AccountName>().expect("a name");
    let mut uow = database.begin().await.expect("begin");
    let owner = uow.users().provision(&owner_did).await.expect("provision");
    let heir = uow.users().provision(&heir_did).await.expect("provision");
    let (account, membership) =
        Account::open(owner.id.clone(), account_did, handle, name, Utc::now());
    uow.accounts()
        .create(&account, &membership)
        .await
        .expect("create");
    uow.commit().await.expect("commit");
    let account_id = AsRef::<str>::as_ref(&account.id).to_owned();
    seat(&pool, &account_id, AsRef::<str>::as_ref(&heir.id), "member")
        .await
        .expect("seat the heir");

    let mut uow = database.begin().await.expect("begin");
    uow.accounts()
        .transfer_ownership(&owner.id, &heir.id, &account.id)
        .await
        .expect("the transfer passes the index");
    uow.commit().await.expect("commit");

    assert_eq!(owners_of(&pool, &account_id).await, 1, "one Owner is left");
    let heir_role: String = sqlx::query_scalar(
        "SELECT role FROM account_members WHERE account_id = $1 AND user_id = $2",
    )
    .bind(&account_id)
    .bind(AsRef::<str>::as_ref(&heir.id))
    .fetch_one(&pool)
    .await
    .expect("read the heir's role");
    assert_eq!(heir_role, "owner", "the heir is the Owner");
}
