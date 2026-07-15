//! `just gen-queries` — regenerate both adapters' `src/queries.rs`.
//!
//! The Zurfur-specific runner around the extracted `sqlx-rust-codegen`
//! library: boots a throwaway PostgreSQL (testcontainers), runs the real
//! embedded migration set, then describes every statement under each crate's
//! `queries/` tree and rewrites its committed `src/queries.rs`. The workspace
//! build never needs this to run — the output is committed; the
//! `codegen_current` test fails loudly (with a diff) when it's stale.

use std::path::PathBuf;

use anyhow::Context;
use testcontainers_modules::{postgres::Postgres, testcontainers::runners::AsyncRunner};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let container = Postgres::default()
        .start()
        .await
        .context("postgres container should start (is the container runtime up?)")?;
    let port = container.get_host_port_ipv4(5432).await?;
    let database_url = format!("postgres://postgres:postgres@127.0.0.1:{port}/postgres");

    let pool = adapter_pg::connect(&database_url).await?;
    adapter_pg::migrate(&pool).await?;
    let mut conn = pool.acquire().await?;

    let config = query_codegen::config();
    let crates_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    for adapter in ["adapter-pg", "adapter-atproto"] {
        let queries = crates_dir.join(adapter).join("queries");
        let generated = sqlx_rust_codegen::generate(&mut conn, &queries, &config).await?;
        let dest = crates_dir.join(adapter).join("src/queries.rs");
        std::fs::write(&dest, generated)
            .with_context(|| format!("cannot write {}", dest.display()))?;
        println!("regenerated {}", dest.display());
    }
    Ok(())
}
