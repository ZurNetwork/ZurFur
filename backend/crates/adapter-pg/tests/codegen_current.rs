//! The staleness gate for the generated query modules: regenerate both crates'
//! `src/queries.rs` against a **freshly migrated** database and diff the result
//! against what is committed.
//!
//! This is the whole verification story of prepare-at-codegen in one test: the
//! regeneration itself `describe`s every statement against the real schema (so a
//! query that no longer prepares fails *here*, with the file named), and the
//! diff proves the committed typed functions — the code every call site
//! compiled against — were generated from the *current* SQL and schema. A stale
//! artifact fails loudly with instructions; it can never false-green the way the
//! retired `.sqlx` cache could, because the comparison source is the live
//! schema, not a snapshot. Comparison is whitespace-insensitive so formatting
//! churn never masquerades as drift. Requires a container runtime socket
//! (DOCKER_HOST honored).

use std::path::PathBuf;

use adapter_pg::PgPool;
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

/// Formatting-insensitive comparison key: rustfmt reflow (whitespace) and the
/// trailing commas it inserts are not drift; any other token change is.
fn normalized(source: &str) -> String {
    source
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ',')
        .collect()
}

#[tokio::test]
async fn committed_query_modules_match_a_fresh_regeneration() {
    let (pool, _container) = fresh_pool().await;
    let mut conn = pool.acquire().await.expect("acquire");

    let crates_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    for adapter in ["adapter-pg", "adapter-atproto"] {
        let queries = crates_dir.join(adapter).join("queries");
        let regenerated = query_codegen::generate(&mut conn, &queries)
            .await
            .unwrap_or_else(|e| panic!("{adapter}: regeneration failed: {e:#}"));

        let committed_path = crates_dir.join(adapter).join("src/queries.rs");
        let committed = std::fs::read_to_string(&committed_path)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", committed_path.display()));

        assert!(
            normalized(&regenerated) == normalized(&committed),
            "{adapter}/src/queries.rs is STALE: the SQL files, annotations, or schema \
             changed since it was generated. Run `just gen-queries` and commit the result."
        );
    }
}
