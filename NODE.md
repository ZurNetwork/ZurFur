---
path: .
charted: 2026-09-12
fs:
  - name: backend/
    role: Rust workspace: domain core + application layer + adapters + the axum api and zurfur cli drivers
    node: true
  - name: frontend/
    role: the SvelteKit web app (frontend/web)
    node: true
  - name: contract/
    role: protobuf API contract, authoritative over both tiers
    node: true
  - name: lexicons/
    role: atproto lexicon JSON (app.zurfur.*) + hermetic validation test
    node: true
  - name: docs/
    role: confluence-design-index.md — pointer index into Confluence DESIGN, never content
    node: true
  - name: forks/
    role: the /boardroom skill's numbered deliberation runs (briefs, seat transcripts, sealed rankings)
    node: true
  - name: scripts/
    role: dev-loop shell backing just recipes + the jj/session hooks
    node: true
  - name: docker/
    role: Dockerfiles for dev-loop-only services (local did:plc directory)
    node: true
  - name: caddy/
    role: Caddyfile — the single dev origin: /api/v1→axum, rest→SvelteKit
    node: true
  - name: reports/
    role: committed third-party (Maigret) HTML reports; unrelated to the build
    node: true
  - name: .claude/
    role: repo-local Claude settings and agents
    node: false
  - name: .github/
    role: CI workflow (fmt/clippy/test/contract/deny/typos/web) + copilot-instructions.md
    node: false
  - name: .githooks/
    role: pre-commit fmt+clippy gate (jj users: scripts/jj-push.sh)
    node: false
  - name: Justfile
    role: task runner — the dev loop (just dev/up/gate/test/migrate-add/gen-*)
    node: false
  - name: Cargo.toml
    role: workspace root; members = backend/crates/*; all dep versions pinned here
    node: false
  - name: Cargo.lock
    role: committed workspace lockfile
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
  - name: .gitattributes
    role: marks the committed generated trees linguist-generated (collapsed in GitHub diffs)
    node: false
  - name: .vscode/
    role: rust-analyzer settings + the `usecase` Rust snippet
    node: false
  - name: .gitmodules
    role: one submodule: proto/googleapis
    node: false
  - name: .chartignore
    role: dirs /chart skips
    node: false
  - name: .gitignore
    role: also honored by /chart; ignores .env*, .idea/, .cursor/, .mcp.json, WORKSHEET.md, target/
    node: false
---
**Is:** Zurfur — an AT Protocol-native art-commission platform: a Rust ports-and-adapters backend, a SvelteKit frontend, and the protobuf contract + atproto lexicons that sit between them and the network.

**Reading this tree:** to understand a path, read every `NODE.md` from this root down to that directory; each one states only what its ancestors haven't. Entries marked `node: false` are fully described by their `fs` line. `/familiarize` reads the tree; `/chart` rewrites it. `NODE.md` is *what/where*; `CLAUDE.md` is *how to behave*.

**Conventions:** all design lives in Confluence DESIGN (single source of truth; `docs/confluence-design-index.md` is the pointer index) — fetch before asserting; Jira ZMVP holds the ticket record. **Doc comments exist in a vacuum:** a `///`, `//!`, JSDoc or SQL-header comment states its fact plainly and never carries a DD number, Confluence id/URL, Jira key, PR number or "per ruling" pointer — those live in the nearest `NODE.md` under **Refs**; documentation too big for its file (module essays, migration rationale) moves to `NODE.md` with a one-line summary left in place. Private data (app-owned rows, Postgres) vs public data (user-owned records, the PDS) is THE boundary; no cross-store transactions. Generated code (backend `api/src/generated`, frontend `lib/server/api/generated`, `adapter-*/src/queries.rs`) is committed and drift-gated in CI — never hand-edit; `just gen-contract` / `just gen-queries` regenerate it, and a SQL comment change re-generates `queries.rs` too. `main` is protected; slice PRs into `feature/*`, squash to `main`.

**Entry points:** `Justfile` → `backend/crates/api/src/main.rs` (server) · `backend/crates/cli/src/main.rs` (terminal) · `frontend/web/src/hooks.server.ts` (web) · `contract/README.md` (wire).

**Refs:** CLAUDE.md · docs/confluence-design-index.md · DESIGN "Domains and Applications" (11763713) · "Data Boundaries" (10354698) · memory `feedback_docs_functions_only_terse` (the docstring rule, 2026-08-29 + 2026-09-12).
