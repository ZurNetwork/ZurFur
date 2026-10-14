//! The shared-container Postgres harness: one container per test process, a
//! migrated template database, and a private clone per test via
//! `CREATE DATABASE … TEMPLATE …`. See this directory's NODE.json for the
//! lifecycle/refcounting details.

use std::sync::{Arc, Mutex, Weak};

use sqlx::{Connection as _, PgConnection};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, ImageExt, runners::AsyncRunner},
};

/// Name of the migrated template database inside the shared container.
const TEMPLATE: &str = "zurfur_template";

/// The Postgres image tag the shared container boots — pinned to match
/// `docker-compose.yml`'s production Postgres (`postgres:16-alpine`), NOT
/// `testcontainers_modules::postgres::Postgres`'s own crate default
/// (`11-alpine`, five majors behind): the suite was silently validating
/// migrations against a Postgres production never runs, discovered when a
/// prototype migration used PG12+-only syntax the crate default can't parse.
/// Tests now run against the same major version production does.
const POSTGRES_TAG: &str = "16-alpine";

/// The per-process shared container plus the coordinates to clone from it.
struct SharedPg {
    /// Held only for its `Drop`: the last `Arc` owner reaps the container.
    _container: ContainerAsync<Postgres>,
    /// Admin URL (the stock `postgres` database) used for `CREATE DATABASE`.
    admin_url: String,
    /// The container's id, for running a client inside it.
    container_id: String,
    /// Serializes clones: Postgres rejects concurrent copies of one template.
    create: tokio::sync::Mutex<()>,
}

/// Rejoin point for the shared container. `Weak`, so holding the static never
/// keeps the container alive on its own — see the module docs on lifecycle.
static SHARED: Mutex<Weak<SharedPg>> = Mutex::new(Weak::new());

/// Serializes the boot path so racing tests can't start two containers.
/// A tokio mutex is safe across the per-`#[tokio::test]` runtimes.
static BOOT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// A private clone of the migrated template database.
///
/// Keep it alive for the test's duration (it also keeps the shared container
/// alive). The database itself is not dropped on teardown — it dies with the
/// container.
pub struct TestDb {
    url: String,
    /// The database's name inside the shared container.
    name: String,
    /// Keeps the shared container alive, and names it.
    shared: Arc<SharedPg>,
}

impl TestDb {
    /// Connection URL of this test's private database.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The command line that runs `psql` inside the container on this database, reading
    /// SQL from stdin and printing bare, unaligned tuples. Needs the `docker` CLI.
    pub fn psql_command(&self) -> String {
        let container = &self.shared.container_id;
        let name = &self.name;
        format!("docker exec -i {container} psql -U postgres -d {name} -tA")
    }
}

/// A fresh, fully migrated, private database for one test.
pub async fn fresh_db() -> TestDb {
    create_db(Some(TEMPLATE)).await
}

/// A fresh, **empty** private database — no migrations applied — for tests
/// that drive the migrator themselves (e.g. stepwise backfill tests).
pub async fn bare_db() -> TestDb {
    create_db(None).await
}

async fn create_db(template: Option<&str>) -> TestDb {
    let shared = shared().await;
    let name = format!("t_{}", uuid::Uuid::now_v7().simple());
    let from = template
        .map(|t| format!(r#" TEMPLATE "{t}""#))
        .unwrap_or_default();
    {
        let _serialize = shared.create.lock().await;
        let mut admin = PgConnection::connect(&shared.admin_url)
            .await
            .expect("admin connection for clone");
        // AssertSqlSafe: identifiers can't be bind parameters; all parts are
        // harness-generated (a uuid and constants), no external input.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"CREATE DATABASE "{name}"{from}"#
        )))
        .execute(&mut admin)
        .await
        .expect("create the test database");
        admin.close().await.ok();
    }
    let (base, _) = shared
        .admin_url
        .rsplit_once('/')
        .expect("admin url has a database segment");
    TestDb {
        url: format!("{base}/{name}"),
        name,
        shared,
    }
}

/// Convenience: [`fresh_db`] plus a connected adapter-pg pool on it.
pub async fn fresh_pool() -> (adapter_pg::PgPool, TestDb) {
    let db = fresh_db().await;
    let pool = adapter_pg::connect(db.url()).await.expect("pool connects");
    (pool, db)
}

/// The live shared container, booting it (and building the template) if this
/// test is first — or the first after a drain.
async fn shared() -> Arc<SharedPg> {
    if let Some(live) = SHARED.lock().expect("shared pg lock").upgrade() {
        return live;
    }
    let _booting = BOOT.lock().await;
    // Re-check under the boot lock: a racer may have finished booting while
    // this test waited.
    if let Some(live) = SHARED.lock().expect("shared pg lock").upgrade() {
        return live;
    }

    let container = Postgres::default()
        .with_tag(POSTGRES_TAG)
        .start()
        .await
        .expect("postgres container should start");
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("mapped postgres port");
    let admin_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");

    // Build the template: create, migrate, then fully disconnect — a template
    // can only be copied while it has no connections.
    let mut admin = PgConnection::connect(&admin_url)
        .await
        .expect("admin connection for template");
    sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"CREATE DATABASE "{TEMPLATE}""#
    )))
    .execute(&mut admin)
    .await
    .expect("create the template database");
    admin.close().await.ok();
    let (base, _) = admin_url.rsplit_once('/').expect("admin url shape");
    let template_url = format!("{base}/{TEMPLATE}");
    let pool = adapter_pg::connect(&template_url)
        .await
        .expect("template pool connects");
    adapter_pg::migrate(&pool).await.expect("migrations run");
    pool.close().await;

    let container_id = container.id().to_owned();
    let live = Arc::new(SharedPg {
        _container: container,
        admin_url,
        container_id,
        create: tokio::sync::Mutex::new(()),
    });
    *SHARED.lock().expect("shared pg lock") = Arc::downgrade(&live);
    live
}
