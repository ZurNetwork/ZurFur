set dotenv-load := true

default:
    @just --list

# --- Dev workflow ---

dev:
    just up
    just _wait-for-db
    just dev-back & just dev-auth & wait

dev-back:
    cd backend && cargo watch -x run

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

# --- Testing ---

# Run all tests (unit + integration). Requires: just up
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
