# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Zurfur is an AT Protocol-native art commission platform built in Rust.

**All design lives in Confluence — it is the single source of truth.** The DESIGN space (https://zurnetwork.atlassian.net/wiki/spaces/DESIGN) holds the glossary (per-entity pages: User, Account, Character, Commission, Golem, Plugin, …), the architecture ("Domains and Applications"), and scope ("Project MVP"). Work is tracked in the Jira project ZMVP. Do not create local design documents; consult and update Confluence instead.

## Commands

All commands use `just` (Justfile at repo root, `dotenv-load` enabled).

```bash
just dev                   # Start everything: Docker, backend + auth frontend
just up                    # Start PostgreSQL via Docker Compose
just down                  # Stop containers
just dev-back              # cargo watch -x run (from backend/)
just check                 # bacon (background type checker, from backend/)
just db-shell              # psql into the running database
just migrate-add <name>    # Create a migration file in adapter-pg
just db-reset              # Drop the DB volume, bring up fresh PostgreSQL
just test                  # cargo test --workspace (integration tests need a container runtime socket, not `just up`)
just setup                 # First-time setup: copy .env, install tools
```

Building and running directly:
```bash
cargo build                          # Build all crates (workspace root)
cargo run -p api                     # Run the API server
cargo test --workspace               # Run all tests
```

## Architecture

Ports and adapters, per the Confluence page "Domains and Applications":

```
backend/crates/
  domain/            # Pure domain elements (Account, User, Golem, Character, Commission, …); will define ports (traits) named by role
  adapter-pg/        # Private data boundary: PostgreSQL (app-owned rows, UUIDv7 keys, transactions)
  adapter-atproto/   # Public data boundary: the user's PDS (user-owned records, AT-URI via DID)
  adapter-mem/       # Both boundaries faked in-process; core development runs against this
  api/               # Composition root: config, tracing, HTTP; the only crate that knows which adapter is live
```

**Dependency rule:** adapters depend on domain crates, never the reverse; `api` composes. The single `domain` crate is transitional — it splits into per-domain crates (`identity`, `gallery`, `workflow`, `plugin`) as those namespaces get built.

Conventions: Rust edition 2024; workspace-level dependency versions in root `Cargo.toml` (add a dependency there only when a crate actually consumes it).

## Configuration

Loaded by figment in `api`: `backend/config/{profile}.toml` first, then `ZURFUR_*` environment variables (env wins). Profile selected by `ZURFUR_ENV` (default `dev`).

Variables: `ZURFUR_ENV`, `ZURFUR_HTTP_ADDR` (default `127.0.0.1:3621`; dev.toml sets `127.0.0.1:8080`), `RUST_LOG` (overrides the `log_level` config), `DATABASE_URL` (deliberately unprefixed — sqlx tooling reads this exact name).

## Database

PostgreSQL 16 via Docker Compose (port 5432, user: admin, db: zurfur). The binary builds a connection pool from `DATABASE_URL` at boot and fails fast if the database is unreachable. Migrations live in `backend/crates/adapter-pg/migrations/`, are embedded via `sqlx::migrate!`, and run automatically on every boot. `GET /health` reports database reachability (200 up / 503 down).

## Branch Strategy

- `main` — stable; all feature PRs target this; **never push directly to `main`**
- `feature/*` — individual units of work, branched from `main`
