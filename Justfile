set dotenv-load := true

default:
    @just --list

# --- Dev workflow ---

dev:
    just up
    just _wait-for-db
    just _wait-for-pds
    just dev-back & just dev-auth & wait

dev-back:
    cargo watch -C backend -x run

dev-auth:
    cd frontend/auth && yarn dev

# --- Docker ---

up:
    docker compose up -d

down:
    docker compose down

logs:
    docker compose logs -f

# --- Database ---

db-shell:
    docker compose exec db psql -U admin -d zurfur

# Create a new migration file in adapter-pg (applied automatically on app boot)
migrate-add name:
    cd backend/crates/adapter-pg && sqlx migrate add {{ name }}

# Drop the database volume and bring up a fresh PostgreSQL.
# `down -v` removes this project's named volumes (project = COMPOSE_PROJECT_NAME,
# default `zurfur`) — that's Postgres AND the dev PDS/PLC (ZMVP-102) together,
# so it does the right thing in an isolated worktree too.
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
    docker compose down -v plc pds
    docker compose up -d plc pds
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

# --- Testing ---

# Run all tests (unit + integration). Integration tests manage their own
# PostgreSQL via testcontainers — needs a container runtime socket
# (DOCKER_HOST is honored; podman works), not `just up`.
test:
    cargo test --workspace

# Regenerate both adapters' src/queries.rs (typed query functions) from the
# queries/*.sql trees, described against a throwaway migrated Postgres. Run
# after adding/changing a .sql file or a migration, then commit the result —
# the codegen_current test fails CI while the committed output is stale.
gen-queries:
    cargo run -p query-codegen

# --- Worktrees (parallel branches) ---

# Seed an isolated .env (unique DB + HTTP/proxy ports + compose project name)
# for the CURRENT git worktree, so `just dev`/`just up`/tests here never collide
# with another worktree's stack. Idempotent — safe to re-run. See /start --worktree.
worktree-init:
    bash scripts/worktree-init.sh

# --- Code quality ---

check:
    cd backend && bacon

# The local mirror of CI's gate (fmt, clippy, test, deny, typos, spec-lint) -- sequential
# and fail-fast, so a red step stops the run before the next one starts. The
# last two need `cargo install cargo-deny` / `cargo install typos-cli` once.
gate:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --locked -- -D warnings
    cargo test --workspace --locked
    cargo deny --locked --all-features check
    typos
    npx --yes @redocly/cli@2.35.1 lint "openapi/*.yaml"

# --- Setup ---

setup:
    @echo "Copying .env.example to .env (edit values before running)..."
    @cp -n .env.example .env || true
    @echo "Installing tools..."
    cargo install just cargo-watch bacon
    cargo install sqlx-cli --no-default-features --features postgres
    cd frontend/auth && yarn install
    @echo ""
    @echo "Done! Edit .env with your secrets, then run: just dev"

clean:
    cargo clean
    rm -rf frontend/auth/node_modules

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
