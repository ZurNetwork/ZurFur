//! The shared-container Postgres harness (ZMVP-134): one container per test
//! process, a fully migrated template database, and a byte-for-byte private
//! clone per test via `CREATE DATABASE … TEMPLATE …`.
//!
//! Replaces the container-per-test pattern (a boot + full migration replay per
//! test function) with a clone that costs tens of milliseconds, without
//! weakening isolation: every test still gets its own pristine database.
//!
//! Lifecycle: the container is **refcounted**, not static. Each [`TestDb`]
//! holds an `Arc` to the shared container; a `Weak` in a process-wide static
//! lets later tests rejoin it. The last live handle reaps the container on
//! drop (testcontainers has no ryuk-style reaper, so a never-dropped static
//! would leak a running container past process exit). If the set of live
//! tests briefly drains to zero mid-run, the next test simply boots a fresh
//! container — correct, just slower for that one boot.
//!
//! Requires a container runtime socket (DOCKER_HOST honored), like the
//! per-test pattern it replaces.

use std::sync::{Arc, Mutex, Weak};

use sqlx::{Connection as _, PgConnection};
use testcontainers_modules::{
    postgres::Postgres,
    testcontainers::{ContainerAsync, runners::AsyncRunner},
};

/// Name of the migrated template database inside the shared container.
const TEMPLATE: &str = "zurfur_template";

/// The per-process shared container plus the coordinates to clone from it.
struct SharedPg {
    /// Held only for its `Drop`: the last `Arc` owner reaps the container.
    _container: ContainerAsync<Postgres>,
    /// Admin URL (the stock `postgres` database) used for `CREATE DATABASE`.
    admin_url: String,
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
    _shared: Arc<SharedPg>,
}

impl TestDb {
    /// Connection URL of this test's private database.
    pub fn url(&self) -> &str {
        &self.url
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
        _shared: shared,
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

    let live = Arc::new(SharedPg {
        _container: container,
        admin_url,
        create: tokio::sync::Mutex::new(()),
    });
    *SHARED.lock().expect("shared pg lock") = Arc::downgrade(&live);
    live
}
