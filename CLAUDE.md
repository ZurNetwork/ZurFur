# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

**Way of working: teams.** The main session is the Orchestrator: it loads the `teams:orchestrate` skill at the start of each session and again after compaction, and follows it. A Spawn (any subagent) skips this line and follows its brief.

Zurfur's teams settings are in `.claude/TEAMS.md`. Where each former CLAUDE.md ruling now lives is recorded in the corpus page `2026101601` "Where Each Zurfur Ruling Lives — the Ways Harvest". This file keeps only Zurfur's own rules.

- **No agent board.** Agents report by hand-back, and the Engineer talks to the Orchestrator in the CLI (or Remote Control when away) (ruling R-118). The agent board is discontinued (the Engineer, 2026-10-16): even if its tools appear, nobody registers, locks, posts or runs rounds on it.
- **Private papers.** Working papers (rulings, handoffs, reviews, retros, release documents) live on the private design corpus's `understand` branch, checked out at `/home/zuri/code/zurfur-workspaces/orchestrator/.understand`, and never in ZurFur, which is public. The Engineer's queue is `queue.md` in Zurfur's Claude project memory folder, `/home/zuri/.claude/projects/-home-zuri-code-zurfur/memory/queue.md` (the Engineer, 2026-10-16). The teams pack's "Documents in git" rule (SKILL.md §4) does not apply here: release documents are committed on the `understand` branch, never as a `docs/<version>` PR into ZurFur. `scripts/private-papers-check.sh` fails on any path, in any letter case, that holds a `.understand/`, `.teams/`, `releases/` or `retros/` folder, a `.claude/projects/` folder (a Claude project or memory folder), a `rulings-*` or `handoff-*` file, a `queue.md` or `engineer-queue.md`, or a `docs/<version>/` folder. `just gate` and CI's `hooks` job run it on the tracked files; `jj push` runs it (`--unpushed`) on every commit the push could send that origin lacks — each commit between origin and the bookmarks, tags or revisions the push names, plus `@` — checking each path a commit adds or changes, so a paper added and later deleted still fails.

## Project

Zurfur is an AT Protocol-native art commission platform built in Rust. ZurFur is to go private and closed source; libraries Zurfur builds (Vulpes and the like) may be open, each licensed when published, and no license file goes on ZurFur. Until it goes private, every public-repo rule here holds. The protos and the lexicons Zurfur publishes will be open, their license chosen when the first lexicon is published (R-312, R-318, R-336, R-338, 2026-10-24).

**All design lives in the design corpus — it is the single source of truth.** The corpus is the sibling repo `ZurNetwork/zurfur-design`, checked out at `~/code/zurfur-design` (`ZURFUR_DESIGN_DIR` overrides the path). One Markdown page per file, named `<id>-<slug>.md`, under `entities/` (the glossary: User, Account, Character, Commission, Golem, Plugin, …), `decisions/` (the DDs), `project/` (scope — "Project MVP" — philosophy, conventions), `data-layer/`, and `superseded/` (pages that govern nothing anymore). Do not create local design documents in this repo; read and update the corpus instead. Confluence DESIGN is a frozen read-only archive — it is neither read nor written.

**Read before guessing.** Many decisions live only in the corpus, not in code. When unsure whether something is already decided/defined and it sounds familiar, **read the relevant design page before asking or asserting from memory.** A generated pointer index of every page (id, status, title) is at @docs/design-index.md — match the topic there, then open that page in the corpus, or `rg` the corpus for the wording. Pages under `superseded/` are hidden from ripgrep by the corpus `.ignore`; open one by path when you need the history behind a live page. If the corpus isn't readable in this session, say so and stop — never fall back to Confluence or memory. Only ask once the page doesn't resolve it, or when a genuinely new decision is needed (route through `/design-decision`).

## Memory & references

- **References, not copies.** The corpus is the source of truth; memory and every `CLAUDE.md` hold **pointers** (page id + title), never copied page bodies. When in doubt, **read** the page (via the index above) rather than assert from memory.
- The folder `CLAUDE.md` files in `backend/crates/adapter-pg` and `backend/crates/adapter-atproto` point at their memories and DDs; honour them.
- Tend this layer with `/optimize-memory` (design pages need no pointer memory — they are in the corpus and in the generated index).

## Decisions and the corpus

- Each merged change is reported to the Engineer with its PR and jj change id.
- **The Engineer reviews no code** *(rulings R-247 to R-249, 2026-10-23, until the Engineer says otherwise)*: the Orchestrator merges PRs into `main` (ZurFur's, the design corpus's and Vulpes's) once the top-level QA (Owl, on Gemini) has reviewed the PR's head, with CI and CodeRabbit's review still gating as before. A one-way door still goes to the Engineer as a plain design question with no code, and its merge waits for the Engineer's yes. That yes, given in chat and recorded in the rulings file, is the door's signature; any migration that changes the shape of stored data is a door, and one that deletes the Engineer's real data always is. Domain choices inside a contract (the protos) come as plain questions too; the contract text itself gets QA (R-313, R-314, R-328, 2026-10-24).
- A general question, once ruled, lands in the corpus and is never re-asked; one that keeps coming back wasn't settled and deserves a DD.
- **Big decision → offer a DD.** When a domain decision the Engineer makes is substantial — shapes an entity, sets an invariant, a real fork with lasting consequences — **offer to capture it as a Design Decision (DD) page** in the corpus's `decisions/` via `/design-decision`. Claude offers; the Engineer confirms before anything is written.
- **Decisions/gaps → offer to update the corpus.** When gaps surface and the Engineer decides how to resolve them, **offer to fix the affected design pages to match those decisions** via `/corpus-sync`. The corpus is the single source of truth, so every decision and gap-resolution must land there — on the Engineer's confirmation, never silently.

## Definition of Done

The teams pack's §8 applies, plus **design in sync**: if a documented entity/flow changed, the design corpus matches the Engineer's decisions (`/corpus-sync`); a big decision is captured as a DD (`/design-decision`); and each feature ships with its moderation hooks: scanning, the hide hold, blocks, the report button and its kill switch (R-35, 2026-10-10).

## Code style (semantic)

Rust code follows the Engineer's **semantic style rulebook** — design page `37519361` "Code Style — Semantic Rulings (Rust)", `project/37519361-code-style-semantic-rulings-rust.md` in the corpus (a living page; read it, don't quote from memory). The short of it: domain-meaningful primitives behind newtypes; multi-line constructions named into a `let` first (tests too); `ok_or_else`/`let-else`/`map_err` over match-plumbing; clarity beats brevity. Binding for all NEW code. Formatting stays rustfmt's job.

Read page `37519361` before writing **or reviewing** Rust. **Standard traits first:** `From`/`TryFrom`/`FromStr`/`Display`/`Default`/`IntoIterator`/`AsRef` are the default vocabulary for conversion, parsing and validation; a custom trait or an inherent `try_new`-style constructor is the justified exception. In reviews, an unjustified custom shape where a std trait fits is a finding.

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
just setup                 # First-time setup: copy .env, install tools (typos, cargo-deny at CI's versions)
just gate                  # The full local mirror of CI (see "Branches and gates")
just gate-light            # The light gate for base merges and fix rounds
just frontend-install      # The guarded frontend install: the guard, the frozen install with scripts skipped, the guard again
just gallery               # Build and serve the component gallery at http://127.0.0.1:5190 (no Docker)
just tools-check           # Warn when local typos/cargo-deny/buf/just/nodes/rustc differ from CI's pins
just npm-advisories        # osv-scanner over both npm lockfiles, each with its root ignore file (needs the network; gate's last step)
just npm-lock-review <rev> # The 7-day floor, the registry integrity check and the licence rule (npm-licences.json) on what both npm lockfiles gained since <rev> (needs the network)
just npm-licences          # Every entry of both npm lockfiles against npm-licences.json's licences (needs the network; in just gate)
just cargo-config-check    # Fail on any tracked path with a .cargo segment (a file, folder or symlink)
```

Building and running directly:
```bash
cargo build                          # Build all crates (workspace root)
cargo run -p api                     # Run the API server
cargo test --workspace               # Run all tests
```

To review a PR locally, the Engineer installs `review-pr` once with `just engineer-tools-install`, then runs `review-pr --start --pr=<N>` in `~/code/zurfur` (`--pause`, `--publish` to post the `REVIEW:` notes as a pending review, `--stop`, `--help`; see the `engineer-tools` node).

## The node tree (`NODE.inf.json`)

Every directory that carries information beyond its parent has a committed `NODE.inf.json` — *what/where*, while `CLAUDE.md` is *how to behave*. A node's `short` is the glance (what `just nodes tree` prints); `is` is the full sentence. Every ref (a node's pointer to a design page) lives in the root `NODE.refs.json`, under its node's path, written with `just nodes add-ref` / `rm-ref`. Node changes ship in the same PR as the code they describe. The tree is read and written through the `nodes` tool (github.com/ZurNetwork/nodes, pinned by `NODES_VERSION` in the Justfile; `just setup` installs it): `just nodes tree` (the whole tree), `just nodes chain <path>` (root → … → node — what `/nodes:familiarize` loads for a path), `just nodes get <path> [field]`, `just nodes refs <pageId>` (every node citing a design page — `/corpus-sync`'s re-point step), `just nodes find <term>`, and for writes `just nodes set <path> <field> '<json>'` · `just nodes add-ref <path> <page> "<title>" "<governs>"` (quote multi-word arguments; they pass through intact) · `rm-ref` · `touch`. A PostToolUse hook (`.claude/settings.json` → `scripts/nodes-fmt-hook.sh`) formats any `NODE.inf.json` or `NODE.refs.json` a model writes; `just nodes-check` (inside `just gate` and `just gate-light`, and CI's `nodes` job) checks the full tree against the schema and `docs/design-index.md`, then its guards: a node tree is present, every `NODE.inf.json` and `NODE.refs.json` is tracked, and it fails on any `NODE.json` outside hidden and build folders (which nodes skips). That index is generated from the corpus by `just design-index`, and `just design-index-check` (also inside `just gate`) reports when it has drifted from the corpus. Never hand-edit `docs/design-index.md`: regenerate it with `just design-index` and keep `just design-index-check --strict` green. *Temporary, until the last branch cut before the move to nodes 0.7 has caught up:* such a branch catches up first (`scripts/nodes-0-7-edits.sh`, whose header has the recipe) and writes no node before then.

**Doc comments exist in a vacuum.** A `///`, `//!`, JSDoc, SQL-header or `.proto` comment states its fact plainly and never carries a DD number, design page id, Jira key, PR number or "per ruling" pointer — every code→design pointer lives in `NODE.refs.json`, under the nearest node's path. Documentation too big for its file (module essays, migration rationale) moves to that node's `notes`, with a one-line summary left in place.

## Architecture

Ports and adapters, per the design page "Domains and Applications" (`11763713`):

```
backend/crates/
  shared/            # Build-time constants every layer may read, the domain crates included
  kernel/            # The base domain crate: the clock, the string builder, Did, text, maturity, BlobId, Markdown
  identity/          # User, Account, Character, actor identities, handles, roles, invitations, profiles, public records, bug reports; their ports
  workflow/          # Commission (elements, slots, seats, changelog, files) and Workflow boards; their ports
  gallery/           # Posts and the Gallery (a stub crate until the first post type)
  moderation/        # Platform safety (a stub crate until the first moderation type)
  domain/            # The façade: re-exports the domain crates as `domain::…`; owns the cross-domain unit of work
  atta/              # The Den's engine: depends on nothing in the workspace; application is its one consumer
  application/       # Use cases: one `#[use_case]` per command or query, called by every driver
  adapter-pg/        # Private data boundary: PostgreSQL (app-owned rows, UUIDv7 keys, transactions)
  adapter-atproto/   # Public data boundary: the user's PDS (user-owned records, AT-URI via DID)
  adapter-mem/       # Both boundaries faked in-process; core development runs against this
  composition/       # Composition root: config and the port bag; the only crate that knows which adapter is live
  api/  cli/         # The HTTP and terminal drivers
```

**Dependency rule:** adapters depend on domain crates, never the reverse; `composition` composes, and `api` and `cli` drive it. Ports are named by role (`PrivateStore`, `PublicRecords`, per-domain repos); crates are named by tech — so a second backend never makes a name a lie. The domain is split into per-domain crates over the base crate `kernel` (`identity`, `workflow`, `gallery`, `moderation`, with `plugin` later), and the thin `domain` façade re-exports the ones that hold types, so every consumer names `domain::…` and nothing else. Domain crates point one way (kernel ← identity ← workflow), each may also read `shared`, and none depends on the façade, an adapter or a driver. Where `gallery` and `moderation` sit is decided with their first types. `domain/tests/it/domain_crates_point_one_way.rs` fails otherwise.

**No cross-store transactions:** anything touching both boundaries (e.g. lock private facts in PostgreSQL + publish an atproto record) is a dual write, run as a separate retryable step (outbox-style), never one unit of work.

**A pure-data core, a command engine, a thin UI** *(ruling 2026-10-12, for every team)*. Zurfur is built the way Photoshop is: the model is pure data, every change is a named command and every read a named query, and each UI (the web frontend, the CLI, an embedded editor such as Zurfur Studio, plugins, agents) is a thin peer adapter that renders and issues commands but never decides. Nothing should be reachable only through one screen. The section is on the design page "Domains and Applications" (`11763713`).

Conventions: Rust edition 2024; workspace-level dependency versions in root `Cargo.toml` (add a dependency there only when a crate actually consumes it).

## Configuration & database

Config is figment in `api`: `backend/config/{profile}.toml` then `ZURFUR_*` env (env wins), profile via `ZURFUR_ENV`. Postgres runs via Docker Compose; migrations live in `backend/crates/adapter-pg/migrations/` (embedded `sqlx::migrate!`, run on boot); `GET /health` reports DB reachability. The env-var catalogue is `.env.example` (every `ZURFUR_*` variable and `DATABASE_URL`, with comments) and `backend/config/{profile}.toml`; check them when touching config, ports, or the DB.

**Always create migrations with `just migrate-add <name>` — never hand-write the filename/version.** The recipe runs `sqlx migrate add`, which stamps a to-the-second UTC version; a hand-typed round timestamp (`…190000`) collides with another branch's migration on the version primary key, and the break only surfaces at rebase/integration (sqlx keys migrations by that number). If you ever see a migration whose `HHMMSS` is a round hour/half-hour, it was hand-authored — suspect a latent collision.

## Branches and gates (Zurfur's specifics)

- `bug/{slug}` branches work like `feature/` ones. CI runs on PRs into `feature/**` by design, so slice branches are named `feature/…`: their children then trigger it too.
- **The working-state contract lives on the feature branch (and `main`)**: every first-parent commit *on the integration branch* (each slice's merge commit) is gate-green (fmt, clippy, full test suite) — incomplete is fine, broken never. Slice branches may hold work-in-progress that doesn't stand alone; the enforcement seam is the merge: a slice PR's CI runs against the merge preview (feature + slice), so green slice CI ⇒ the feature branch stays working after the merge.
- Stacked slice PRs: a merge commit keeps the merged slice's commits, so the next slice is still built on them and needs no base merge; GitHub retargets its PR to the feature branch when the merged slice's branch is deleted. When a slice must pick up something new from the feature branch, merge the feature branch into it (a base merge): Claude's branches never rewrite a pushed branch. Cross-branch restacks stay the Engineer's. (Engineer rulings 2026-10-11 and 2026-10-19.)
- **PRs into `main` go through the merge queue** *(Engineer ruling 2026-10-11)*. Once a PR's required checks pass, `gh pr merge <N> --squash` queues it (before they pass, the same command turns on auto-merge, which queues it when they do). The queue re-runs CI on the PR applied to the latest `main` and the PRs ahead of it, then squash-merges; nobody clicks "Update branch" or rebases just to catch up with `main`. PRs into `feature/*` don't use the queue.
- **CodeRabbit reviews PRs into `main` only** (the Engineer, 2026-10-16). It is part of the top-level review layer, beside QA and the architects, and a merge into `main` waits for its review: queue the merge, or turn on auto-merge, only once CodeRabbit's review is on the PR and the top-level QA has reviewed its head (R-247). On a PR into `main`, `scripts/pr-wait.sh` waits for that review of the PR's head: a CodeRabbit review of the head commit, or CodeRabbit's summary comment saying its review up to the head found nothing to fix. Its status check alone doesn't count, since it turns green on skipped heads too. Exit 5 means CI is green but CodeRabbit didn't review the head ("Review skipped", a rate limit or a pause): tell the Engineer. With no review by `PR_WAIT_TIMEOUT` (45 minutes), it exits 2. PRs into other branches don't wait for CodeRabbit. `main`'s ruleset blocks a merge while any review thread is open, CodeRabbit's included: resolve a thread once it is fixed or declined with a reason, and leave one awaiting the Engineer's ruling open. CodeRabbit's status is deliberately not a required check: it never reports on the merge queue's commits, so the queue would stall. Dependabot's PRs into `main` wait like any other; post one `@coderabbitai review` comment per bot PR to trigger it (R-235, 2026-10-21).
- **Push only with `jj push`** (`scripts/jj-push.sh`). It runs fmt and clippy, then, right before pushing, checks the papers over everything the push could send, and it pushes to origin only. It refuses `-c` and `--named` for any change but `@`, and every option it can't check (`-R`, `--at-op`, `--config`, …). Raw `git push` and `jj git push` skip the papers check: never use them here, because ZurFur is public.
- **npm lockfiles (R-234).** CI's `npm-lock` job checks both npm lockfiles with the base commit's tools and data: osv-scanner against each lockfile's root ignore file, a 7-day floor on every added entry, whose integrity must match the registry's, and the licence rule against the base's `npm-licences.json`; `web`, `ui` and `contract` wait for it. A younger security fix passes only on a waiver already on the base: security reads its diff, a PR adding it to `lock-age-waivers.json` merges first, then a base merge (`@dependabot rebase` on a Dependabot PR) re-runs the check; never push commits to Dependabot's branch, except one reviewed commit on the monthly github-actions PR that updates the action pins in `scripts/tools-check.sh` and `scripts/npm-lock-pins.test.sh`, each new SHA checked against its upstream tag and each version at least 7 days old, as `.github/dependabot.yml`'s header says. A new or renewed acceptance in `osv-scanner.*.toml` lands in its own PR too. A new allowed licence, scope or exception in `npm-licences.json` lands in its own PR first, because CI reads the base's list, and compliance reads it.
- **Local gates.** `just gate` needs the network: `cargo deny`, `just npm-licences` and `just npm-advisories`, its last step, reach it. `just gate-light` runs fmt, clippy, typos, `just design-index-check` (CI can't run it; the corpus is private), `just nodes-check`, the tests of the crates the change touches, and, when `frontend/` or `contract/` changed, the guarded frontend install, the guard's tests and the web and ui checks. `jj push` always runs fmt + clippy. To wait for CI, Copilot and CodeRabbit, run `scripts/pr-wait.sh <PR>` once in the background instead of polling, with `PR_WAIT_COPILOT=0` while Copilot is paused. It waits for Copilot and CodeRabbit only on PRs into `main`. Exit 4 means CI is green but Copilot was unavailable (a quota or no-files notice): go on, and tell the Engineer. Exit 5 is CodeRabbit's (above).

## Workspaces

- **jj; git worktrees are retired** (ruling 2026-07-20). Workspaces live under `/home/zuri/code/zurfur-workspaces`, one per branch (the teams pack's rule); `/home/zuri/code/zurfur` is the Engineer's checkout.
- **Never remove the `orchestrator` workspace.** `/home/zuri/code/zurfur-workspaces/orchestrator` holds the docs folder (`.understand`, the corpus `understand` worktree). No cleanup removes it — not `/teams:cleanup`, not a Machinery Spawn, not the recipe below; only the Engineer retires it.
- **Removing a workspace:** `jj st` in it, `jj workspace forget <name>` from another workspace, then delete the folder, `target/` included. Commits live in the shared repo, so nothing is lost, and a workspace's build folder grows to tens of GB. A Machinery Spawn uses this recipe, not `/teams:cleanup`: that skill fetches with `jj --config …`, which Zurfur's settings deny.
- Tests are isolated either way (each spins up its own testcontainers Postgres on a random port); the **dev stack is not** — a second checkout that runs `just dev` needs its own DB/HTTP ports and compose project name in its `.env` (see `.env.example`).
