//! The one-time delete of every stored atproto OAuth session and pending
//! sign-in: run every migration before it, seed both tables beside Users, an
//! Account, its custody keys and a login session, run the delete and nothing
//! after it, and check that both tables are empty while every other table
//! keeps exactly its rows. Requires a container runtime socket (DOCKER_HOST honored).

use std::collections::BTreeMap;

use adapter_pg::{PgDatabase, PgPool};
use chrono::Utc;
use domain::{
    elements::{
        account::{Account, AccountName},
        did::Did,
        handle::Handle,
        user::User,
    },
    ports::Database,
};

/// The delete's migration, as sqlx numbers it. The test runs everything before
/// it, seeds, then runs it.
const DELETE_STORED_OAUTH_SESSIONS: i64 = 20261012014616;

/// A fresh, empty private database with no migrations applied; the second
/// element keeps the shared container alive for the test's duration.
async fn bare_pool() -> (PgPool, impl Sized) {
    let db = test_support::pg::bare_db().await;
    let pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    (pool, db)
}

/// Run every embedded migration older than the delete.
async fn migrate_to_just_before_the_delete(pool: &PgPool) {
    let mut before = adapter_pg::migrator();
    let names_an_embedded_migration = before
        .migrations
        .iter()
        .any(|migration| migration.version == DELETE_STORED_OAUTH_SESSIONS);
    assert!(
        names_an_embedded_migration,
        "the version constant names an embedded migration"
    );
    let migrations: Vec<_> = before
        .migrations
        .iter()
        .filter(|migration| migration.version < DELETE_STORED_OAUTH_SESSIONS)
        .cloned()
        .collect();
    before.migrations = migrations.into();
    before.run(pool).await.expect("the earlier migrations run");
}

/// Run the delete itself and nothing after it, so a later migration's new
/// table never reads as a change the delete made.
async fn migrate_through_the_delete(pool: &PgPool) {
    let mut through = adapter_pg::migrator();
    let migrations: Vec<_> = through
        .migrations
        .iter()
        .filter(|migration| migration.version <= DELETE_STORED_OAUTH_SESSIONS)
        .cloned()
        .collect();
    through.migrations = migrations.into();
    through.run(pool).await.expect("the delete runs");
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

/// Found an Account owned by `owner` in its own committed unit of work.
async fn found_account(pool: &PgPool, owner: &User) {
    let account_did = Did::from("did:plc:oauthpurgeacct".to_owned());
    let handle: Handle = "oauth-purge.example.com".parse().expect("a handle");
    let name: AccountName = "Purge Studio".parse().expect("a name");
    let (account, membership) =
        Account::open(owner.id.clone(), account_did, handle, name, Utc::now());
    let db = PgDatabase::new(pool.clone());
    let mut uow = db.begin().await.expect("begin");
    uow.accounts()
        .create(&account, &membership)
        .await
        .expect("create");
    uow.commit().await.expect("commit");
}

/// Store one OAuth session row the way the auth store keys it.
async fn seed_session(pool: &PgPool, account_did: &str, session_id: &str) {
    sqlx::query(
        "INSERT INTO atproto_oauth.client_session (account_did, session_id, data)
         VALUES ($1, $2, $3)",
    )
    .bind(account_did)
    .bind(session_id)
    .bind(b"sealed session".as_slice())
    .execute(pool)
    .await
    .expect("seed a stored session");
}

/// Store one pending sign-in, bound to a browser or not.
async fn seed_request(pool: &PgPool, state: &str, browser_binding: Option<&[u8]>) {
    sqlx::query(
        "INSERT INTO atproto_oauth.auth_request (state, data, browser_binding)
         VALUES ($1, $2, $3)",
    )
    .bind(state)
    .bind(b"sealed request".as_slice())
    .bind(browser_binding)
    .execute(pool)
    .await
    .expect("seed a pending sign-in");
}

/// Store one of Zurfur's own login sessions.
async fn seed_login_session(pool: &PgPool) {
    sqlx::query(
        "INSERT INTO tower_sessions.session (id, data, expiry_date)
         VALUES ($1, $2, now() + interval '1 day')",
    )
    .bind("login-session")
    .bind(b"login session".as_slice())
    .execute(pool)
    .await
    .expect("seed a login session");
}

/// Store one Account's wrapped custody keys.
async fn seed_account_keys(pool: &PgPool, did: &str) {
    sqlx::query(
        "INSERT INTO account_keys (did, wrapped_keys, created_at)
         VALUES ($1, $2, now())",
    )
    .bind(did)
    .bind(b"wrapped keys".as_slice())
    .execute(pool)
    .await
    .expect("seed custody keys");
}

/// How many rows `table` holds; `table` is always a test constant.
async fn rows_in(pool: &PgPool, table: &'static str) -> i64 {
    let count = format!("SELECT count(*) FROM {table}");
    sqlx::query_scalar(sqlx::AssertSqlSafe(count))
        .fetch_one(pool)
        .await
        .expect("count rows")
}

/// Every base table outside the catalogs except the two the delete empties and
/// the migration ledger, each with an md5 of all its rows (`None` for an empty
/// table). A table the delete adds, even beside those two, shows up too.
async fn digest_of_every_other_table(pool: &PgPool) -> BTreeMap<String, Option<String>> {
    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT format('%I.%I', table_schema, table_name)
         FROM information_schema.tables
         WHERE table_type = 'BASE TABLE'
           AND table_schema NOT IN ('pg_catalog', 'information_schema')
           AND (table_schema, table_name) NOT IN (
               ('atproto_oauth', 'client_session'),
               ('atproto_oauth', 'auth_request'),
               ('public', '_sqlx_migrations')
           )
         ORDER BY 1",
    )
    .fetch_all(pool)
    .await
    .expect("list the tables");
    let mut digests = BTreeMap::new();
    for table in tables {
        // AssertSqlSafe: `table` is a catalog name quoted by `format('%I')`.
        let digest_query =
            format!("SELECT md5(string_agg(t::text, ',' ORDER BY t::text)) FROM {table} t");
        let digest: Option<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(digest_query))
            .fetch_one(pool)
            .await
            .expect("digest a table");
        digests.insert(table, digest);
    }
    digests
}

#[tokio::test]
async fn the_delete_empties_both_oauth_tables_and_touches_nothing_else() {
    let (pool, _container) = bare_pool().await;
    migrate_to_just_before_the_delete(&pool).await;

    let owner = provision(&pool, "did:plc:oauthpurgeowner").await;
    let other = provision(&pool, "did:plc:oauthpurgeother").await;
    found_account(&pool, &owner).await;
    seed_login_session(&pool).await;
    seed_account_keys(&pool, "did:plc:oauthpurgeacct").await;
    seed_session(&pool, owner.id.as_ref(), "state-owner").await;
    seed_session(&pool, other.id.as_ref(), "state-other").await;
    seed_session(&pool, "did:web:oauth-purge.example.com", "state-web").await;
    seed_request(&pool, "state-pending", Some([9u8; 32].as_slice())).await;
    seed_request(&pool, "state-unbound", None).await;
    let digests_before = digest_of_every_other_table(&pool).await;
    let seeded_tables = [
        "public.users",
        "public.accounts",
        "public.account_members",
        "public.account_keys",
        "tower_sessions.session",
    ];
    for table in seeded_tables {
        let seeded = digests_before.get(table).is_some_and(Option::is_some);
        assert!(seeded, "{table} holds a row before the delete");
    }

    migrate_through_the_delete(&pool).await;

    let sessions_after = rows_in(&pool, "atproto_oauth.client_session").await;
    let requests_after = rows_in(&pool, "atproto_oauth.auth_request").await;
    let digests_after = digest_of_every_other_table(&pool).await;
    assert_eq!(
        sessions_after, 0,
        "every stored session is gone, did:web too"
    );
    assert_eq!(requests_after, 0, "every pending sign-in is gone");
    assert_eq!(
        digests_after, digests_before,
        "every other table keeps exactly its rows"
    );
}
