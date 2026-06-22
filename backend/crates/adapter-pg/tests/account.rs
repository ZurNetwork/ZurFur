//! Round-trips `PgAccountRepo` against a throwaway PostgreSQL container, proving the
//! migration-created `accounts`/`account_members` tables persist a founded account and
//! its Owner membership in one transaction, that `find` reads the account back and hides
//! the unknown, and that `role_of` answers membership. The founder is provisioned through
//! `PgUserRepo` first because `account_members.user_id` references `users(id)`. Requires a
//! container runtime socket (DOCKER_HOST honored).
use adapter_pg::{PgAccountRepo, PgPool, PgUserRepo};
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountName},
        did::Did,
        role::Role,
    },
    ports::{AccountRepo, UserRepo},
};
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};

/// Boots a fresh database and runs migrations. The container is returned so the
/// caller keeps it alive for the test's duration.
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

#[tokio::test]
async fn create_persists_the_account_and_its_owner_membership() {
    let (pool, _container) = fresh_pool().await;
    let users = PgUserRepo::new(pool.clone());
    let accounts = PgAccountRepo::new(pool.clone());

    // The founder must exist: account_members.user_id references users(id).
    let owner = users
        .provision(&Did::new("did:plc:pgowner".to_string()))
        .await
        .expect("provision owner");

    let account_did = Did::new("did:plc:pgacct".to_string());
    let account_name = AccountName::try_new("PG Studio").unwrap();
    let (account, membership) = Account::open(
        owner.id,
        account_did.clone(),
        account_name.clone(),
        Utc::now(),
    );
    let account_id = account.id;
    accounts
        .create(&account, &membership)
        .await
        .expect("create founds the account and its membership");

    let found = accounts
        .find(account_id)
        .await
        .expect("find")
        .expect("the founded account is present");
    assert_eq!(found.id, account_id);
    assert_eq!(
        found.did, account_did,
        "the account's minted did round-trips"
    );
    assert_eq!(found.name, account_name, "the account's name round-trips");
    assert_eq!(found.deleted_at, None, "a freshly founded account is live");

    let role = accounts
        .role_of(owner.id, account_id)
        .await
        .expect("role_of");
    assert_eq!(
        role,
        Some(Role::Owner(None)),
        "the creating User is the account's Owner"
    );
}

#[tokio::test]
async fn find_unknown_account_is_none() {
    let (pool, _container) = fresh_pool().await;
    let users = PgUserRepo::new(pool.clone());
    let accounts = PgAccountRepo::new(pool.clone());

    let owner = users
        .provision(&Did::new("did:plc:pgowner2".to_string()))
        .await
        .expect("provision owner");
    // Founded in the domain but never persisted, so its id is genuinely unknown.
    let (unfounded, _) = Account::open(
        owner.id,
        Did::new("did:plc:ghost".to_string()),
        AccountName::try_new("Ghost").unwrap(),
        Utc::now(),
    );

    let found = accounts.find(unfounded.id).await.expect("find");
    assert!(
        found.is_none(),
        "an account we never founded resolves to nothing"
    );
}

#[tokio::test]
async fn role_of_non_member_is_none() {
    let (pool, _container) = fresh_pool().await;
    let users = PgUserRepo::new(pool.clone());
    let accounts = PgAccountRepo::new(pool.clone());

    let owner = users
        .provision(&Did::new("did:plc:pgowner3".to_string()))
        .await
        .expect("provision owner");
    let stranger = users
        .provision(&Did::new("did:plc:pgstranger".to_string()))
        .await
        .expect("provision stranger");

    let (account, membership) = Account::open(
        owner.id,
        Did::new("did:plc:pgacct3".to_string()),
        AccountName::try_new("PG Studio 3").unwrap(),
        Utc::now(),
    );
    accounts
        .create(&account, &membership)
        .await
        .expect("create");

    let role = accounts
        .role_of(stranger.id, account.id)
        .await
        .expect("role_of");
    assert_eq!(role, None, "a user who is not a member holds no role");
}
