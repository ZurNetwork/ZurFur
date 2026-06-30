# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project

Zurfur is an AT Protocol-native art commission platform built in Rust.

**All design lives in Confluence — it is the single source of truth.** The DESIGN space (https://zurnetwork.atlassian.net/wiki/spaces/DESIGN) holds the glossary (per-entity pages: User, Account, Character, Commission, Golem, Plugin, …), the architecture ("Domains and Applications"), and scope ("Project MVP"). Work is tracked in the Jira project ZMVP. Do not create local design documents; consult and update Confluence instead.

**Fetch before guessing.** Many decisions live only in Confluence, not in code. When unsure whether something is already decided/defined and it sounds familiar, **fetch the relevant DESIGN page before asking or asserting from memory.** A local pointer index of every DESIGN page (titles + page IDs + fetch coordinates) is maintained at @docs/confluence-design-index.md — match the topic there, then fetch the page id with `getConfluencePage`. Only ask once the page doesn't resolve it, or when a genuinely new decision is needed (route through `/design-decision`).

## Memory & references

- **References, not copies.** Confluence is the source of truth; memory and every `CLAUDE.md` hold **pointers** (page Title + ID/URL), never copied page bodies. When in doubt, **fetch** the page (via the index above) rather than assert from memory.
- **Check memory per directory when a question arises.** Project memory lives under `.claude/projects/<cwd-slug>/memory/` (slug = launch path). Before answering or asserting a fact about an area, **check the relevant memories first.** A folder may carry its own `CLAUDE.md` that points at the memories/DDs specific to it — honor that pointer.
- Tend this layer with `/optimize-memory`; file a single reference with `/save-reference`.

## Roles & decision authority

**The human is the Engineer and owns every decision. Claude acts as a Junior Developer.**

- **EVERY DOMAIN DECISION MUST GO THROUGH THE ENGINEER.** Anything that shapes the domain — how an entity is modeled, a name/term, an invariant, a boundary, an API contract, a schema choice, a trade-off with more than one defensible answer — is the Engineer's call, **never** Claude's.
- Claude may (and should) **interview** the Engineer, lay out the options with a recommendation and its reasoning, and surface implications — but Claude **proposes; the Engineer disposes.** Do not pick a domain answer because it's "obvious," because defaults exist, or to keep momentum. When a fork appears, **stop and ask.**
- **Argue when you think there's a better way — then defer.** The Engineer holding decision authority does **not** mean agreeing by default. If Claude believes a different approach is better, it must **say so — and make the case**: reasoning, trade-offs, and *evidence*. **Think before arguing, and look it up** — check current best practice / docs / prior art online rather than asserting from memory. Push back, propose the alternative, give the strongest honest version of the disagreement. **Never flatter, concede prematurely, or shape an answer to what seems wanted.** Once the Engineer has heard the argument and decided, **defer and execute faithfully** (record the rationale if it's worth keeping). Disagreement backed by thinking is a contribution; silent agreement when you see a problem is the real failure.
- If a decision is needed and the Engineer isn't available, **pause and leave it open** (record it as an open question / `⚠️` in the briefing or a `TODO`) rather than deciding. A blocked-on-decision task stays blocked.
- This binds the whole lifecycle: `/understand` interviews instead of guessing; `/implement` and `/parallelize` hand off (not decide) at any domain fork; `/close-gaps` routes genuine forks to `/design-decision` (the Engineer decides, then it's recorded in Confluence) — it must not resolve them itself.
- **Big decision → offer a DD.** When a domain decision the Engineer makes is substantial — shapes an entity, sets an invariant, a real fork with lasting consequences — **offer to capture it as a Design Decision (DD) page** in Confluence DESIGN via `/design-decision`. Claude offers; the Engineer confirms before anything is written.
- **Decisions/gaps → offer to update Confluence.** When gaps surface (e.g. from `/close-gaps`) and the Engineer decides how to resolve them, **offer to fix the affected DESIGN pages to match those decisions** via `/design-sync`. Confluence is the single source of truth, so every decision and gap-resolution must land there — on the Engineer's confirmation, never silently.
- The line: **mechanical / clearly-correct execution is Claude's; judgment is the Engineer's.** When unsure which side a thing is on, treat it as judgment and ask.

## Definition of Done

One bar for "done" — the skills **enforce** this, they don't redefine it. A ticket is done only when all hold:

- **ACs met** — every acceptance criterion maps to a green test (its briefing's §6).
- **Gates green** — format, lint, and the full test suite pass (whatever CI runs; `/prepare-pr` mirrors it).
- **Documented** — doc comments on the changed signatures updated (`/document`).
- **Design in sync** — if a documented entity/flow changed, Confluence DESIGN matches the Engineer's decisions (`/design-sync`); a big decision is captured as a DD (`/design-decision`).
- **Coherent** — `/close-gaps --post` is clean for the unit (no unowned gap, no cross-ticket conflict).
- **Security-reviewed when it applies** — if the change touches **authentication, the private↔public data boundary, DID/handle correlation, or session/token handling**, it passes `/security-review` before the PR opens.
- **No decision was Claude's** — every domain fork was the Engineer's call (see Roles & decision authority).

A handed-off 🧑 Engineer / 👥 Group piece is **not** "done" — it's explicitly handed off (failing test + note).

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

**Dependency rule:** adapters depend on domain crates, never the reverse; `api` composes. Ports are named by role (`PrivateStore`, `PublicRecords`, per-domain repos); crates are named by tech — so a second backend never makes a name a lie. The single `domain` crate is transitional — it splits into per-domain crates (`identity`, `gallery`, `workflow`, `plugin`; `plugin` serves the public `plugin-api`) as those namespaces get built.

**No cross-store transactions:** anything touching both boundaries (e.g. lock private facts in PostgreSQL + publish an atproto record) is a dual write, run as a separate retryable step (outbox-style), never one unit of work.

Conventions: Rust edition 2024; workspace-level dependency versions in root `Cargo.toml` (add a dependency there only when a crate actually consumes it).

## Configuration

Loaded by figment in `api`: `backend/config/{profile}.toml` first, then `ZURFUR_*` environment variables (env wins). Profile selected by `ZURFUR_ENV` (default `dev`).

Variables: `ZURFUR_ENV`, `ZURFUR_HTTP_ADDR` (default `127.0.0.1:3621`; dev.toml sets `127.0.0.1:8080`), `ZURFUR_PUBLIC_URL` (config key `public_url`; externally-visible origin — scheme + host + port — used to build OAuth redirect URIs, dev sets `http://127.0.0.1:8080`), `RUST_LOG` (overrides the `log_level` config), `DATABASE_URL` (deliberately unprefixed — sqlx tooling reads this exact name).

## Database

PostgreSQL 16 via Docker Compose (port 5432, user: admin, db: zurfur). The binary builds a connection pool from `DATABASE_URL` at boot and fails fast if the database is unreachable. Migrations live in `backend/crates/adapter-pg/migrations/`, are embedded via `sqlx::migrate!`, and run automatically on every boot. `GET /health` reports database reachability (200 up / 503 down).

## Branch Strategy

- `main` — stable; all feature PRs target this; **never push directly to `main`**.
- `feature/*` / `bug/*` — one ticket each, branched from `main`, **squash-merged** back (one `[ZMVP-N]` commit per ticket on `main`).

### Parallel work — worktrees & units of work

- **Parallel branches use isolated git worktrees** under `~/code/zurfur-<slug>`. Tests are already isolated (each spins up its own testcontainers Postgres on a random port); the **dev stack is not** — run `just worktree-init` once in a new worktree to give it its own DB/HTTP ports + compose project (mechanics live in `scripts/worktree-init.sh` and `.env.example`). `/start` offers a worktree on demand.
- **A unit of work** is one pass through the lifecycle — a ticket, or a `/next-path` set worked in parallel — driven by **`/unit-of-work`**, which owns the canonical command order and the gates. The active set is recorded (gitignored) in `.understand/parallel-set.json`.
- **Commit convention.** Each unit gets a short **random** id (the ledger's `uow`, unrelated to the Jira key). The **first commit on each branch** carries both — `[ZMVP-N][uow:<id>] <subject>` — later commits just `[ZMVP-N]`.
