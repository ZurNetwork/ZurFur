set dotenv-load := true

default:
    @just --list

# --- Dev workflow ---

dev:
    just up
    just _wait-for-db
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

# Drop the database volume and bring up a fresh PostgreSQL
db-reset:
    docker compose down -v
    -docker volume rm "$(basename "$(pwd)")_pg_data"
    just up
    just _wait-for-db

# --- Testing ---

# Run all tests (unit + integration). Integration tests manage their own
# PostgreSQL via testcontainers — needs a container runtime socket
# (DOCKER_HOST is honored; podman works), not `just up`.
test:
    cargo test --workspace

# --- Code quality ---

check:
    cd backend && bacon

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
