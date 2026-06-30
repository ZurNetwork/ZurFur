---
name: config and runtime reference
description: Zurfur api config (figment + ZURFUR_* env vars, defaults) and Postgres runtime specifics — lookup detail, not always-loaded
metadata:
  type: reference
---

Config/runtime lookup detail for the `api` crate — recalled when touching config, env, ports, or the DB, so it lives here rather than always-loaded in `CLAUDE.md`.

**Loading:** figment in `api` reads `backend/config/{profile}.toml` first, then `ZURFUR_*` environment variables (env wins). Profile via `ZURFUR_ENV` (default `dev`).

**Env vars:**
- `ZURFUR_ENV` — profile selector (default `dev`).
- `ZURFUR_HTTP_ADDR` — default `127.0.0.1:3621`; `dev.toml` sets `127.0.0.1:8080`.
- `ZURFUR_PUBLIC_URL` (config key `public_url`) — externally-visible origin (scheme + host + port), used to build OAuth redirect URIs; dev sets `http://127.0.0.1:8080`.
- `RUST_LOG` — overrides the `log_level` config.
- `DATABASE_URL` — deliberately **unprefixed** (sqlx tooling reads this exact name).

**Postgres:** PostgreSQL 16 via Docker Compose (port `5432`, user `admin`, db `zurfur`). The binary builds a pool from `DATABASE_URL` at boot and **fails fast** if unreachable. Migrations in `backend/crates/adapter-pg/migrations/`, embedded via `sqlx::migrate!`, run on every boot. `GET /health` reports DB reachability (200 up / 503 down).

Per-worktree DB/HTTP port isolation: `scripts/worktree-init.sh` (+ `.env.example`).
