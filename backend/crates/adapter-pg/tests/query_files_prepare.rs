//! The verification gate for the SQL-file convention: every statement in the
//! generated query registries — this crate's and adapter-atproto's (whose
//! `atproto_oauth` schema these migrations create) — must `PREPARE` against the
//! freshly migrated database.
//!
//! This is the replacement for what the retired `sqlx::query!` macros checked at
//! compile time: syntax, table/column existence, and placeholder inference are
//! all validated by Postgres preparing the statement, against the **real**
//! migrated schema (a stale-cache false green is impossible — there is no
//! cache). Iterating `ALL_QUERIES` — not globbing the directory — means the
//! corpus checked is byte-for-byte the corpus the binaries embed. What `PREPARE`
//! cannot see (a row struct's fields matching the selected columns) is covered
//! by the behavior suites, which execute every statement end-to-end. Requires a
//! container runtime socket (DOCKER_HOST honored).

use adapter_pg::PgPool;
use sqlx::{Executor, SqlSafeStr};
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
async fn every_registered_query_prepares_against_the_migrated_schema() {
    let (pool, _container) = fresh_pool().await;
    let mut conn = pool.acquire().await.expect("acquire");

    // adapter-atproto's auth_store queries run against tables these migrations
    // create (it owns no DDL), so this one gate covers both crates' registries.
    let corpora: [(&str, Vec<(String, &'static str)>); 2] = [
        (
            "adapter-pg",
            adapter_pg::queries::ALL_QUERIES
                .iter()
                .map(|q| (format!("{q:?}"), q.sql()))
                .collect(),
        ),
        (
            "adapter-atproto",
            adapter_atproto::queries::ALL_QUERIES
                .iter()
                .map(|q| (format!("{q:?}"), q.sql()))
                .collect(),
        ),
    ];

    let mut checked = 0usize;
    for (crate_name, corpus) in corpora {
        for (name, sql) in corpus {
            conn.prepare(sql.into_sql_str()).await.unwrap_or_else(|e| {
                panic!("{crate_name} {name} does not prepare against the migrated schema: {e}")
            });
            checked += 1;
        }
    }

    // A hollowed-out registry must fail loudly, not pass by preparing nothing:
    // the floor is well below the real corpus but far above an empty scan.
    assert!(
        checked >= 70,
        "prepared suspiciously few registered queries ({checked}) — check the registries"
    );
}
