---
path: backend/crates/query-codegen
charted: 2026-09-12
fs:
  - name: Cargo.toml
    role: manifest
    node: false
  - name: src/
    role: config + runner binary (lib.rs config, main.rs bin)
    node: false
---
**Is:** the Zurfur-specific runner around the extracted `sqlx-rust-codegen` library — boots a throwaway Postgres, migrates it with the real embedded migration set, describes every statement under `adapter-pg`'s and `adapter-atproto`'s `queries/` trees, and rewrites each crate's committed `src/queries.rs`.

**Conventions:** the `codegen_current` staleness test that shares this crate's config lives in the adapter crates, not here. The workspace build never invokes this binary — it runs only on demand.

**Entry points:** `src/main.rs` (`just gen-queries`, boots testcontainers Postgres and regenerates both adapters); `src/lib.rs` (`config()`, the shared `sqlx-rust-codegen::Config`).

**Refs:** none — no Confluence/Jira pointers found in this crate's doc comments.
