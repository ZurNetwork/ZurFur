---
path: backend
charted: 2026-08-21
fs:
  - name: config/
    role: figment profile TOML (dev.toml): env, bind addr, public_url, log level, PDS endpoint
    node: false
  - name: crates/
    role: the workspace members
    node: true
---
**Is:** The Rust backend: a pure `domain` core implemented by a private Postgres boundary, a public AT-Protocol boundary, and in-memory fakes, composed by one axum `api` binary.

**Conventions:** no `backend/Cargo.toml` — the workspace root is the repo root. Edition 2024, every crate `publish = false`. Dependency rule one-way: adapters → domain, never reverse; only `api` names concrete adapters. Ports named by role (`UserStore`, `PublicRecords`), crates by tech (`adapter-pg`). Private-store writes are a compile-enforced capability: only via `Database::begin()` → `UnitOfWork` (DD 24150017). Config = `config/{profile}.toml` then `ZURFUR_*` env (env wins), profile via `ZURFUR_ENV`. Tests self-provision infrastructure (testcontainers Postgres / throwaway PDS) — a container socket, not `just up`.

**Entry points:** `crates/api/src/main.rs` · `crates/api/src/lib.rs` (Config, AppState, `app()`) · `config/dev.toml`.

**Refs:** memory `config-and-runtime` · DD "Transactions as a capability" (24150017).
