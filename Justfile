set dotenv-load := true
# Recipe arguments reach the shell as "$@", so `just nodes add-ref … "a title" "text (with parens)"`
# passes every argument through intact; `{{ARGS}}` would re-split and re-parse them.
set positional-arguments := true

# The `nodes` tool (github.com/ZurNetwork/nodes — the node-tree normalizer and
# lookup CLI), pinned to one release tag. Bump it HERE and nowhere else: the
# node files (NODE.inf.json, with every ref in the root NODE.refs.json) and the
# tool's schema move in lockstep on this pin. Exported so scripts/tools-check.sh
# reads it.
export NODES_VERSION := "v0.7.1"

# The tool versions CI runs, so the local gate agrees with it. `just setup`
# installs typos, cargo-deny and just at these; `just tools-check` warns when this
# machine differs and fails when .github/workflows disagrees. The workflows
# pin the same versions (typos and cargo-deny through their actions' SHAs);
# bump both sides in one change. Rust is pinned in rust-toolchain.toml.
export TYPOS_VERSION := "1.51.1"
export CARGO_DENY_VERSION := "0.20.2"
export BUF_VERSION := "1.72.0"
export JUST_VERSION := "1.49.0"

# The design corpus (github.com/ZurNetwork/zurfur-design — private; the single
# source of truth for Zurfur's glossary, decisions, scope and architecture),
# checked out as a sibling of this repo. Override for a different checkout.
export ZURFUR_DESIGN_DIR := env("ZURFUR_DESIGN_DIR", env("HOME") / "code" / "zurfur-design")

default:
    @just --list

# --- Dev workflow ---

# Every dev recipe below writes its output to logs/<name>/<UTC stamp>.log at the
# checkout's root as well as the terminal (scripts/dev-log.sh; plain text, the
# newest 10 kept, logs/latest/<name>.log the newest). `just up` also follows each
# compose service into logs/compose/<service>/. `just logs-tail [name]` follows
# them; `just logs-clean` removes them.

# ZURFUR_BUILD_ID names the build in every bug report: scripts/build-id.sh's
# jj or git commit, " (local changes)" when the working copy differs, else
# "unknown". dev, dev-back, dev-web, dev-mock, dev-profile and dev-profile-web
# export it only when it is unset, so a parent recipe's value wins (`dev`'s
# dev-back and dev-web share one) and so does your shell's. It is stamped when
# the recipe starts; cargo watch and vite reloads keep it until you restart the
# recipe. Never put it in .env: dotenv-load would push one stale value into
# every recipe.
_export_build_id := 'export ZURFUR_BUILD_ID="${ZURFUR_BUILD_ID:-$(bash scripts/build-id.sh)}"'

# The whole stack: compose, then the API and the web. It logs as `dev`, and the
# recipes it runs log for themselves too.
dev:
    {{_export_build_id}} && bash scripts/dev-log.sh dev just _dev

_dev:
    just up
    just _wait-for-db
    just _wait-for-pds
    just dev-back & just dev-web & wait

dev-back:
    # -p api: the workspace has a second binary (contract-gen) since ZMVP-25;
    # a bare `run` is ambiguous and cargo-watch dies silently under `just dev`.
    {{_export_build_id}} && bash scripts/dev-log.sh dev-back cargo watch -C backend -x 'run -p api'

# The primary SvelteKit web app (frontend/web) behind Caddy's catch-all (ZMVP-150).
dev-web:
    {{_export_build_id}} && cd frontend/web && bash ../../scripts/dev-log.sh dev-web yarn dev

# Mock-mode dev loop (ZMVP-198): the SvelteKit app alone, no axum, no Docker,
# no Caddy — ZurfurApi is served by the in-memory mock Layer (runtime.ts's
# seam branch), boot signed in, and /mock/signin completes the login form
# without OAuth. Deliberately skips `just up`/wait-for-*: there is nothing to
# wait for.
dev-mock:
    {{_export_build_id}} && cd frontend/web && ZURFUR_WEB_MOCK=1 ZURFUR_PUBLIC_URL="http://127.0.0.1:${ZURFUR_WEB_PORT:-5174}" bash ../../scripts/dev-log.sh dev-mock yarn dev

# The dev profile (no Docker, no PostgreSQL, no PDS): the API on the in-memory
# fixture world, where every sign-in is alice. The `dev-profile` Cargo feature
# alone chooses it (nothing in .env can); the binary refuses to start unless
# ZURFUR_ENV=dev, which this recipe sets. It binds `http_addr` (default
# 127.0.0.1:8181) and builds its redirects and CSRF check on `public_url`
# (default http://127.0.0.1:8180, the origin a web run in front of it uses),
# clear of `just dev`'s 8080 (Caddy), 8081 (axum) and 5174 (web), so both can
# run at once; their sessions never cross. It builds into target/dev-profile,
# so target/debug/api is never the profile. Override either:
# `just dev-profile 127.0.0.1:9191 http://127.0.0.1:9190`.
dev-profile http_addr="127.0.0.1:8181" public_url="http://127.0.0.1:8180":
    {{_export_build_id}} && ZURFUR_ENV=dev ZURFUR_HTTP_ADDR="$1" ZURFUR_PUBLIC_URL="$2" CARGO_TARGET_DIR=target/dev-profile bash scripts/dev-log.sh dev-profile cargo run -p api --features dev-profile

# The dev profile in a browser: its one origin (the `caddy-dev-profile` compose
# service, the same Caddyfile, loopback only) on `proxy_port`, in front of the
# dev-profile API at `api_upstream` and the web's vite dev server on `web_port`,
# which runs in the foreground like `dev-web`; Ctrl-C stops both. Run
# `just dev-profile` in another terminal, then browse http://127.0.0.1:8180 and
# sign in (every sign-in is alice). The defaults mirror `dev-profile`'s, so it
# runs beside `just dev` without colliding, though cookies don't isolate by
# port: signing in here signs `just dev`'s browser session out (neither stack
# accepts the other's session). Needs .env (`just setup`): compose
# reads it, and so does the API. Other ports, with the API to match:
# `just dev-profile 127.0.0.1:8281 http://127.0.0.1:8280` and
# `just dev-profile-web 8280 http://127.0.0.1:8281 5284`.
dev-profile-web proxy_port="8180" api_upstream="http://127.0.0.1:8181" web_port="5184":
    {{_export_build_id}} && bash scripts/dev-log.sh dev-profile-web just _dev-profile-web "$1" "$2" "$3"

_dev-profile-web proxy_port api_upstream web_port:
    #!/usr/bin/env bash
    set -euo pipefail
    export ZURFUR_DEV_PROFILE_PROXY_PORT="$1" ZURFUR_DEV_PROFILE_API_UPSTREAM="$2" ZURFUR_DEV_PROFILE_WEB_PORT="$3"
    trap 'docker compose --profile dev-profile down caddy-dev-profile' EXIT
    docker compose --profile dev-profile up -d caddy-dev-profile
    (cd frontend/web && ZURFUR_WEB_PORT="$3" ZURFUR_API_UPSTREAM="$2" ZURFUR_PUBLIC_URL="http://127.0.0.1:$1" ZURFUR_WEB_STACK=dev-profile yarn dev)

# The frontend workspace's one install: the guard over the manifests and
# frontend/yarn.lock, the frozen install with every package script skipped, then
# the guard over what was installed. CI's install steps are these three lines.
frontend-install:
    node frontend/install-check.mjs
    yarn --cwd frontend install --frozen-lockfile --ignore-scripts
    node frontend/install-check.mjs --installed

# The @zurfur/ui component gallery: install, build, then serve the build with
# vite preview. frontend/ui/vite.config.ts alone sets the host and port
# (127.0.0.1, ZURFUR_GALLERY_PORT or 5190). No Docker, no API, no .env.
gallery:
    just frontend-install
    yarn --cwd frontend/ui run build
    yarn --cwd frontend/ui run preview

# --- Docker ---

# Compose up, then one log follower per service from just before it
# (scripts/compose-logs.sh), replacing the services' old followers.
up:
    bash scripts/dev-log.sh up just _up

_up:
    #!/usr/bin/env bash
    set -euo pipefail
    since="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    docker compose up -d
    bash scripts/compose-logs.sh start "$since"

# Compose down; the services' log followers stop with it.
down:
    docker compose down
    bash scripts/compose-logs.sh stop

logs:
    docker compose logs -f

# Follow the dev logs: every logs/latest/*.log, or only <name>'s (a recipe such
# as dev-back, or a compose service such as db). An unknown name lists the names.
logs-tail name="":
    bash scripts/logs.sh tail "$1"

# Stop the compose log followers, then remove logs/.
logs-clean:
    bash scripts/logs.sh clean

# --- Database ---

db-shell:
    docker compose exec db psql -U admin -d zurfur

# Fill your Den on `just dev` with two commissions, one archived, as you: sign
# in, then paste your zurfur.sid cookie's value at the hidden prompt. Run it
# once: each run adds two more commissions. It sends ZURFUR_PUBLIC_URL as its Origin.
den-fill:
    bash scripts/den-fill.sh

# Create a new migration file in adapter-pg (applied automatically on app boot)
migrate-add name:
    cd backend/crates/adapter-pg && sqlx migrate add {{ name }}

# Drop the database volume and bring up a fresh PostgreSQL.
# `down -v` removes this project's named volumes (project = COMPOSE_PROJECT_NAME,
# default `zurfur`) — that's Postgres AND the dev PDS/PLC (ZMVP-102) together,
# so it does the right thing in an isolated workspace too.
db-reset:
    docker compose down -v
    just up
    just _wait-for-db
    just _wait-for-pds

# --- Local PDS + PLC (dev loop; ZMVP-102) ---

# Wipe ONLY the dev PDS + local PLC back to clean, leaving Postgres alone.
# The PLC has no volume at all (it's an in-memory mock DB — recreating the
# container already wipes it); `pds_data` is the PDS's own named volume, torn
# down here with `down -v` (service-scoped — `rm -v` isn't supported by every
# compose provider; `down -v <service>` is).
pds-reset:
    #!/usr/bin/env bash
    set -euo pipefail
    docker compose down -v plc pds
    since="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    docker compose up -d plc pds
    bash scripts/compose-logs.sh start "$since" plc pds
    just _wait-for-pds

# Create (or, if it already exists, sign into) the fixture test account on the
# local dev PDS and print the resulting session as JSON. Idempotent — safe to
# run again after a `pds-reset`/`db-reset`, or against an already-provisioned
# account.
pds-provision:
    bash scripts/pds-provision.sh

# Scripted proof for ZMVP-102: boot -> provision -> sign-in -> wipe -> clean,
# plus the no-public-egress guarantee (AC4). NOT a cargo test — the automated,
# per-test-throwaway PDS harness is ZMVP-103's lane; this proves the *dev
# loop* itself.
pds-smoke:
    bash scripts/pds-smoke-test.sh

# --- Web dev loop (ZMVP-150) ---

# Scripted proof for ZMVP-150: through the Caddy public origin, assert the
# one-origin split routes /api/* (stripped) to axum, the auth carve-outs
# (/signin-callback, /.well-known/*) to axum verbatim, / to SvelteKit, and that
# /apifoo never reaches the backend. Needs the stack already running (`just dev`)
# — axum and the vite dev server run on the host, so there's nothing to boot here.
web-smoke:
    bash scripts/web-smoke.sh

# --- Testing ---

# Run all tests (unit + integration). Integration tests manage their own
# PostgreSQL via testcontainers — needs a container runtime socket
# (DOCKER_HOST is honored; podman works), not `just up`.
test:
    cargo test --workspace

# The terminal driving adapter (epic ZMVP-199): `just zurfur -- session whoami`.
zurfur *ARGS:
    cargo run --quiet --package cli -- "$@"

# The bug reports filed from the web, oldest first, through the operator command
# `zurfur bug-reports list`: fenced text with every invisible character escaped,
# or `--json` (printable ASCII). `just bug-reports --since 2026-10-17T00:00:00Z`.
bug-reports *ARGS:
    cargo run --quiet --package cli -- bug-reports list "$@"

# Regenerate both adapters' src/queries.rs (typed query functions) from the
# queries/*.sql trees, described against a throwaway migrated Postgres. Run
# after adding/changing a .sql file or a migration, then commit the result —
# the codegen_current test fails CI while the committed output is stale.
gen-queries:
    cargo run -p query-codegen

# Regenerate BOTH tiers' committed generated code from the contract corpus
# (DD 40992770): the api crate's src/generated/ (drift-gated by the
# contract_current test) and the frontend's protobuf-es output (drift-gated by
# the CI contract job's git-diff step). Needs the `buf` CLI for the frontend
# half (https://buf.build/docs/installation).
gen-contract:
    cargo run -p contract-gen
    cd contract && buf generate

# Regenerate the Den's golden responses under contract/golden/den/ from the
# in-memory fixture world. Run after a contract or fixture change, then commit;
# the den_golden test (in `just gate` and CI) fails while they are stale.
den-golden:
    DEN_GOLDEN=write cargo test -p api --test den_golden

# Clock times of each pair of not-found causes on PostgreSQL, from a warm and a
# cold profile cache, printed as a Markdown table for the security review. Not a
# gate. DEN_TIMING_RUNS sets the timed runs per cause (default 200); extra
# arguments go to cargo (e.g. `just den-timing --release`).
den-timing *ARGS:
    cargo test -p api --test not_found_timing {{ARGS}} -- --ignored --nocapture


# --- The node tree (NODE.inf.json, NODE.refs.json) ---

# The per-directory node chart (one NODE.inf.json per directory; every ref lives
# in the root NODE.refs.json), through the pinned `nodes` binary:
# `just nodes tree` · `just nodes chain <path>` · `just nodes get <path> [field]`
# · `just nodes refs <pageId>` · `just nodes find <term>` · `just nodes fmt [file…]`
# · `just nodes set <path> <field> <json>` · add-ref / rm-ref / touch (`nodes --help`).
# Needs the binary: `just nodes-install` (run by `just setup`).
nodes *ARGS:
    nodes "$@"

# Validate every NODE.inf.json and NODE.refs.json: the schema, path ↔ location,
# fs ↔ child nodes, and every ref against the design pointer index (a SUPERSEDED
# entry warns). Then scripts/nodes-tree-check.sh guards what `nodes check` passes
# over: a stray NODE.json, no node tree here (a refs-only check), an untracked
# NODE.inf.json, an untracked NODE.refs.json. Part of `just gate`; CI runs it as
# the `nodes` job.
nodes-check:
    nodes check --ref-index docs/design-index.md
    sh scripts/nodes-tree-check.sh

# Download the pinned `nodes` release binary into ~/.cargo/bin (checksum-verified;
# falls back to `cargo install --git` when no prebuilt archive fits this machine).
nodes-install:
    sh scripts/nodes-install.sh {{NODES_VERSION}}

# --- The design corpus (ZurNetwork/zurfur-design) ---

# Regenerate the public pointer index from the design corpus at $ZURFUR_DESIGN_DIR.
design-index:
    cargo run -q --manifest-path "$ZURFUR_DESIGN_DIR/Cargo.toml" -p design -- index --root "$ZURFUR_DESIGN_DIR" > docs/design-index.md

# Check the committed pointer index against the corpus. Part of `just gate`;
# CI runs it in the `nodes` job. See scripts/design-index-check.sh for the
# ruled drift semantics (migration/DECISIONS.md "Drift check", plan P6).
design-index-check *ARGS:
    sh scripts/design-index-check.sh "$@"

# Unit tests for design-index-check.sh's own drift classification (fixture-
# driven; never compiles the real design crate). CI runs it in the `nodes` job.
design-index-check-test:
    sh scripts/design-index-check.test.sh

# Run every Claude Code hook script's test suite (scripts/hooks/*.test.sh —
# fixture-driven, no live Claude session needed). Part of `just gate`; CI
# runs it in the `hooks` job.
hooks-test:
    #!/usr/bin/env bash
    set -euo pipefail
    status=0
    for t in scripts/hooks/*.test.sh; do
        echo "--- $t ---"
        sh "$t" || status=1
    done
    exit "$status"

# Fixture tests for the CI path classifier, the `changes` job, gate-light, the
# dev-profile pins, pr-wait, tools-check, the nodes install check, the nodes fmt
# hook, den-fill, the dev logs, the private-papers check, the cargo-config
# check, the node-tree guard, the nodes 0.7 node-text edits, the `jj push`
# gate's argument guard, workspace-init, the npm advisory scan, CI's npm-lock
# driver and its pins (the design-index check's own tests run under
# `design-index-check-test`). CI runs it in the `hooks` job, which has no jj
# and no nodes: the jj fixtures and the node-text edits' tests skip themselves
# there.
scripts-test:
    #!/usr/bin/env bash
    set -euo pipefail
    status=0
    for t in scripts/ci-paths.test.sh scripts/ci-changes.test.sh scripts/gate-light.test.sh scripts/dev-profile-pins.test.sh scripts/pr-wait.test.sh scripts/tools-check.test.sh scripts/nodes-install.test.sh scripts/nodes-fmt-hook.test.sh scripts/den-fill.test.sh scripts/dev-log.test.sh scripts/compose-logs.test.sh scripts/logs.test.sh scripts/private-papers-check.test.sh scripts/cargo-config-check.test.sh scripts/nodes-tree-check.test.sh scripts/nodes-0-7-edits.test.sh scripts/jj-push.test.sh scripts/workspace-init.test.sh scripts/build-id.test.sh scripts/npm-advisories.test.sh scripts/npm-lock-ci.test.sh scripts/npm-lock-pins.test.sh; do
        echo "--- $t ---"
        sh "$t" || status=1
    done
    exit "$status"

# Scan frontend/yarn.lock and lexicons/package-lock.json for published
# advisories with the pinned osv-scanner, each against its own root ignore file
# (osv-scanner.frontend.toml, osv-scanner.lexicons.toml). Needs the network:
# the package names and versions go to osv.dev, and the first run downloads the
# scanner from GitHub releases. `just gate` runs it last.
npm-advisories:
    sh scripts/npm-advisories.sh

# The 7-day floor over both npm lockfiles against jj revision <rev>: its
# frontend/yarn.lock (else its frontend/web/yarn.lock, else an empty one) and
# lexicons/package-lock.json (else an empty one) are the old lockfiles, and
# this checkout's frontend/lock-review.mjs and lock-age-waivers.json judge
# every entry added since. Needs the network (the npm registry). CI's npm-lock
# job runs the same floor with the PR base's tools and waivers.
npm-lock-review rev:
    #!/usr/bin/env bash
    set -uo pipefail
    rev="$1"
    files="$(jj file list -r "$rev")" || { echo "npm-lock-review: can't list the files at $rev" >&2; exit 2; }
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    # old_lock <out> <empty content> <path…>: the first <path> <rev> holds, else <empty content>.
    old_lock() {
        local out="$1" empty="$2" path
        shift 2
        for path in "$@"; do
            if printf '%s\n' "$files" | grep -qxF -- "$path"; then
                jj file show -r "$rev" -- "$path" >"$out" || exit 2
                echo "old lockfile: $rev's $path"
                return
            fi
        done
        printf '%s\n' "$empty" >"$out"
        echo "old lockfile: empty ($rev has none of $*)"
    }
    old_lock "$tmp/frontend" $'# THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.\n# yarn lockfile v1' frontend/yarn.lock frontend/web/yarn.lock
    old_lock "$tmp/lexicons" '{"lockfileVersion": 3, "packages": {"": {}}}' lexicons/package-lock.json
    worst=0
    for pair in "frontend frontend/yarn.lock" "lexicons lexicons/package-lock.json"; do
        set -- $pair
        echo "floor over $2"
        node frontend/lock-review.mjs "$tmp/$1" "$2" --min-age-days 7 --waivers lock-age-waivers.json
        code=$?
        if [ "$code" -gt "$worst" ]; then worst="$code"; fi
    done
    exit "$worst"

# Compare this machine's typos, cargo-deny, buf, just, nodes and rustc with CI's versions
# (warns), and check every workflow pins its actions by SHA at the Justfile's
# versions (fails). `--pins-only` skips the local half; CI runs that.
tools-check *ARGS:
    sh scripts/tools-check.sh "$@"

# Parse the TOML file CI's `changes` job counts as docs-only (NODE.toml, when
# present), so a broken one still fails CI. Needs python3 >= 3.11 (tomllib).
toml-check:
    python3 -c 'import os, sys, tomllib; [tomllib.load(open(f, "rb")) for f in sys.argv[1:] if os.path.exists(f)]' NODE.toml

# Fail when a tracked path looks like a private working paper (ignoring case:
# a .understand/, .teams/, releases/ or retros/ folder, a rulings-* or
# handoff-* file, a queue.md or engineer-queue.md file, or a docs/<version>/
# folder): those live on the design corpus's `understand` branch, and this
# repo is public. Part of `just gate`; CI runs it in the `hooks` job. `jj push`
# runs its --unpushed mode instead, which checks what each commit not on
# origin that the push names adds or changes.
private-papers-check:
    sh scripts/private-papers-check.sh

# Fail when a .cargo/config or .cargo/config.toml is tracked (any depth, any case).
cargo-config-check:
    sh scripts/cargo-config-check.sh

# Forward to the design corpus's recall eval harness (private, Python
# stdlib) at $ZURFUR_DESIGN_DIR/eval/run.py — see that repo for subcommands
# (lint, smoke, pilot, matrix). Fails with a clear message if the corpus
# checkout has no eval/ yet.
eval *ARGS:
    #!/usr/bin/env bash
    set -euo pipefail
    runner="$ZURFUR_DESIGN_DIR/eval/run.py"
    if [ ! -f "$runner" ]; then
        echo "just eval: $runner not found — the design corpus checkout at \$ZURFUR_DESIGN_DIR has no eval/ harness yet" >&2
        exit 1
    fi
    python3 "$runner" "$@"

# --- Workspaces (parallel branches) ---

# Seed an isolated .env (unique DB + HTTP/proxy ports + compose project name)
# for the CURRENT jj workspace, so `just dev`/`just up`/tests here never collide
# with another workspace's stack. Idempotent — safe to re-run.
workspace-init:
    bash scripts/workspace-init.sh

# --- Code quality ---

check:
    cd backend && bacon

# The local mirror of CI's gate (fmt, clippy, test, deny, typos, contract, web, ui).
# CI's contract job ADDITIONALLY runs (a) `buf breaking` against the PR base —
# a comparison that only makes sense per-PR — and (b) the frontend codegen
# drift check (clean-slate `buf generate` + index diff); both deliberately
# CI-only here. The backend half of the drift gate DOES run locally, inside
# `cargo test` (the contract_current test); regenerate both tiers with
# `just gen-contract` before pushing a corpus change.
# -- sequential and fail-fast, so a red step stops the run before the next one
# starts. Needs `just setup` once (typos and cargo-deny at CI's versions), a
# one-time `yarn --cwd frontend/ui playwright install chromium` for web's and
# ui's browser-mode component tests (one hoisted Playwright serves both), and
# the `buf` CLI at BUF_VERSION for the contract lint
# (https://buf.build/docs/installation — a single Go binary).
# The npm lockfiles' 7-day floor is CI-only (its npm-lock job): it compares
# against a PR's base; `just npm-lock-review <rev>` runs it by hand.
# Run it before a slice's first push for review and before handing it to QA;
# base merges and fix rounds may use `just gate-light`.
gate:
    just tools-check
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    # The dev-profile build gets its own target dir, so target/debug/api is
    # never the profile; scripts/dev-profile-pins.test.sh pins these lines.
    CARGO_TARGET_DIR=target/dev-profile cargo clippy -p api --all-targets --locked --features dev-profile -- -D warnings
    INSTA_UPDATE=no DEN_GOLDEN=check cargo test --workspace --locked
    CARGO_TARGET_DIR=target/dev-profile INSTA_UPDATE=no cargo test -p api --locked --features dev-profile --lib --test dev_profile
    cargo deny --locked --all-features check
    typos
    just design-index-check
    just nodes-check
    just hooks-test
    just scripts-test
    just toml-check
    just private-papers-check
    just cargo-config-check
    buf lint contract
    just frontend-install
    node --test frontend/install-check.test.mjs frontend/lock-review.test.mjs
    yarn --cwd frontend/web run check
    yarn --cwd frontend/web run lint
    yarn --cwd frontend/web run test
    yarn --cwd frontend/web run build
    yarn --cwd frontend/ui run check
    yarn --cwd frontend/ui run lint
    yarn --cwd frontend/ui run test
    yarn --cwd frontend/ui run build
    just engineer-tools-test
    just npm-advisories

# engineer-tools/review-pr.sh becomes ${XDG_BIN_HOME:-~/.local/bin}/review-pr,
# outside any checkout a review rewrites; it prints the commit it came from.
# Install review-pr on PATH (run again to update).
engineer-tools-install:
    #!/usr/bin/env bash
    set -euo pipefail
    bin="${XDG_BIN_HOME:-$HOME/.local/bin}"
    mkdir -p "$bin"
    from="$(jj log --no-graph --ignore-working-copy -r @ -T 'commit_id.short(12)' 2>/dev/null || git rev-parse --short=12 HEAD 2>/dev/null || echo unknown)"
    install -m 755 engineer-tools/review-pr.sh "$bin/review-pr"
    echo "installed review-pr from $from at $bin/review-pr"
    case ":$PATH:" in
        *":$bin:"*) ;;
        *) echo "note: $bin is not on PATH; add it to run review-pr" ;;
    esac

# Tests for the Engineer's own tools (engineer-tools/*.test.sh); each skips
# when jj isn't installed, as on CI.
engineer-tools-test:
    #!/usr/bin/env bash
    set -euo pipefail
    status=0
    for t in engineer-tools/*.test.sh; do
        echo "--- $t ---"
        sh "$t" || status=1
    done
    exit "$status"

# The light gate for base merges and fix rounds: fmt, clippy, typos and the
# design-index check, plus the tests of the crates the change touches and the
# frontend install, the install guard's tests and the web and ui checks when
# frontend/ or contract/ changed. CI runs the full suite on
# every push; see scripts/gate-light.sh for how the change is read.
# `just gate-light --plan` prints the steps without running them.
gate-light *ARGS:
    bash scripts/gate-light.sh "$@"

# --- Setup ---

setup:
    @echo "Copying .env.example to .env (edit values before running)..."
    @cp -n .env.example .env || true
    @echo "Installing tools..."
    cargo install --locked just --version {{JUST_VERSION}}
    cargo install cargo-watch bacon
    cargo install cargo-insta
    cargo install sqlx-cli --no-default-features --features postgres
    cargo install --locked typos-cli --version {{TYPOS_VERSION}}
    cargo install --locked cargo-deny --version {{CARGO_DENY_VERSION}}
    @echo "Install buf {{BUF_VERSION}}: https://buf.build/docs/installation"
    just nodes-install
    just frontend-install
    @echo ""
    @echo "Done! Edit .env with your secrets, then run: just dev"

clean:
    cargo clean
    rm -rf frontend/web/node_modules frontend/web/.svelte-kit frontend/web/build
    rm -rf frontend/node_modules frontend/ui/node_modules frontend/ui/.svelte-kit frontend/ui/build

# --- Internal ---

_wait-for-db:
    #!/usr/bin/env bash
    echo "Waiting for PostgreSQL..."
    for i in $(seq 1 30); do
        if docker compose exec -T db pg_isready -U admin -d zurfur > /dev/null 2>&1; then
            echo "PostgreSQL is ready."
            exit 0
        fi
        sleep 1
    done
    echo "ERROR: PostgreSQL did not become ready in 30s"
    exit 1

_wait-for-pds:
    #!/usr/bin/env bash
    echo "Waiting for the local PDS + PLC (ZMVP-102)..."
    for i in $(seq 1 30); do
        if docker compose exec -T plc wget -q -O- http://localhost:2582/_health > /dev/null 2>&1 \
            && docker compose exec -T pds wget -q -O- http://localhost:3000/xrpc/_health > /dev/null 2>&1; then
            echo "PDS + PLC are ready."
            exit 0
        fi
        sleep 1
    done
    echo "ERROR: PDS/PLC did not become ready in 30s"
    exit 1
