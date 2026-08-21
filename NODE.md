---
path: .
charted: 2026-08-21
fs:
  - name: backend/
    role: Rust workspace: domain core + adapters + axum api
    node: true
  - name: frontend/
    role: the SvelteKit web app
    node: true
  - name: contract/
    role: protobuf API contract, authoritative over both tiers
    node: true
  - name: lexicons/
    role: atproto lexicon JSON (app.zurfur.*) + hermetic validation test
    node: true
  - name: docs/
    role: confluence-design-index.md — pointer index into Confluence DESIGN, never content
    node: false
  - name: .claude/
    role: repo-local Claude memory bank (projects/…/memory, indexed by MEMORY.md)
    node: false
  - name: .github/
    role: CI workflow (fmt/clippy/test/contract/deny/typos/web) + copilot-instructions.md
    node: false
  - name: .githooks/
    role: pre-commit fmt+clippy gate (jj users: scripts/jj-push.sh)
    node: false
  - name: scripts/
    role: dev-loop shell: jj-push gate, worktree-init, pds-provision/smoke, web-smoke, uow-status hook
    node: false
  - name: docker/
    role: plc/Dockerfile — local did:plc directory image
    node: false
  - name: caddy/
    role: Caddyfile — single dev origin: /api/v1→axum, rest→SvelteKit
    node: false
  - name: reports/
    role: two committed Maigret HTML reports; unrelated to the build
    node: false
  - name: Justfile
    role: task runner — the dev loop (just dev/up/test/migrate-add/gen-*)
    node: false
  - name: Cargo.toml
    role: workspace root; members = backend/crates/*; all dep versions pinned here
    node: false
  - name: docker-compose.yml
    role: dev stack db/pds/plc/caddy, namespaced by COMPOSE_PROJECT_NAME
    node: false
  - name: deny.toml
    role: cargo-deny policy (CI `deny` job)
    node: false
  - name: typos.toml
    role: spell-check exemptions
    node: false
  - name: CLAUDE.md
    role: how to behave: Confluence-is-truth, roles, DoD, branch strategy
    node: false
  - name: AGENTS.md
    role: generic agent instructions: summary + crate map
    node: false
  - name: .env.example
    role: ZURFUR_* env template (env overrides backend/config/<profile>.toml)
    node: false
  - name: .gitmodules
    role: one submodule: proto/googleapis
    node: false
  - name: .chartignore
    role: dirs /chart skips
    node: false
---
**Is:** Zurfur — an AT Protocol-native art-commission platform: a Rust ports-and-adapters backend, a SvelteKit frontend, and the protobuf contract + atproto lexicons that sit between them and the network.

**Reading this tree:** to understand a path, read every `NODE.md` from this root down to that directory; each one states only what its ancestors haven't. Entries marked `node: false` are fully described by their `fs` line. `/familiarize` reads the tree; `/chart` rewrites it. `NODE.md` is *what/where*; `CLAUDE.md` is *how to behave*.

**Conventions:** all design lives in Confluence DESIGN (single source of truth; `docs/confluence-design-index.md` is the pointer index) — fetch before asserting. Work is tracked in Jira ZMVP. Private data (app-owned rows, Postgres) vs public data (user-owned records, the PDS) is THE boundary; no cross-store transactions. Generated code (backend `api/src/generated`, frontend `lib/server/api/generated`, `adapter-*/src/queries.rs`) is committed and drift-gated in CI — never hand-edit. `main` is protected; slice PRs into `feature/*`, squash to `main`.

**Entry points:** `Justfile` → `backend/crates/api/src/main.rs` (server) · `frontend/web/src/hooks.server.ts` (web) · `contract/README.md` (wire).

**Refs:** CLAUDE.md · docs/confluence-design-index.md · DESIGN "Domains and Applications" (11763713) · "Data Boundaries" (10354698).
