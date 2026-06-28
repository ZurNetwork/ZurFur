#!/usr/bin/env bash
#
# worktree-init.sh — give the current git worktree its own isolated runtime.
#
# Every Zurfur integration test already spins up a throwaway PostgreSQL container
# on a RANDOM host port (testcontainers) and binds the HTTP server to :0, so the
# test suite is collision-free across worktrees out of the box. The thing that is
# NOT isolated is the *manual* dev environment: `just up` / `just dev` use the
# fixed host ports 5432 (db) and 8080 (backend), and a single shared DB volume.
# Two worktrees running it at once would fight over those ports and corrupt each
# other's schema.
#
# This script closes that gap. It writes a managed block into THIS worktree's
# `.env` that pins:
#   * COMPOSE_PROJECT_NAME    — namespaces containers + the pg_data volume
#   * ZURFUR_DB_HOST_PORT     — the host port docker-compose maps Postgres to
#   * ZURFUR_PROXY_HOST_PORT  — the host port for the optional nginx proxy
#   * ZURFUR_HTTP_ADDR        — where the backend binds (env wins over dev.toml)
#   * ZURFUR_PUBLIC_URL       — matches the bind addr so OAuth URIs are coherent
#   * DATABASE_URL            — points sqlx/the app at this worktree's DB port
#
# Ports are derived from the worktree's directory name, so they are stable across
# re-runs (a given worktree always gets the same ports) yet distinct per worktree.
# Secrets (JWT/OAuth keys) are copied once from the primary worktree's `.env`.
#
# Idempotent: re-running rewrites the managed block in place and leaves the rest
# of `.env` — including your secrets — untouched. The primary worktree keeps the
# defaults (5432/8080); there is no reason to run this there.

set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

slug="$(basename "$root")"

# The primary worktree is the first entry of `git worktree list`. It holds the
# canonical `.env` with the real secrets; we seed from it so a fresh worktree
# (where `.env` is gitignored and therefore absent) still has working keys.
main_root="$(git worktree list --porcelain | awk '/^worktree /{print $2; exit}')"

if [ "$root" = "$main_root" ]; then
  cat >&2 <<EOF
refusing to isolate the PRIMARY worktree ($slug).
The primary checkout keeps the default ports (db 5432, http 8080). Run this from
a secondary worktree created by \`git worktree add\` (see /start --worktree).
EOF
  exit 1
fi

# --- pick three stable, free host ports -------------------------------------
# Base is a hash of the slug folded into 20000–38999, leaving room for +1/+2.
# We then walk forward past any port that currently has a listener, so a re-run
# while the stack is up still lands on this worktree's own (already-bound) ports
# only if they are free — otherwise it advances, which is fine and deterministic
# enough for local dev.
port_in_use() {
  # Succeeds (returns 0) when something is LISTENING on 127.0.0.1:$1.
  (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null && { exec 3>&- 3<&- 2>/dev/null; return 0; }
  return 1
}

next_free() {
  local p="$1"
  while port_in_use "$p"; do
    p=$((p + 1))
    [ "$p" -gt 65000 ] && p=20000
  done
  printf '%s' "$p"
}

hash="$(printf '%s' "$slug" | cksum | cut -d' ' -f1)"
base=$((20000 + hash % 19000))
db_port="$(next_free "$base")"
http_port="$(next_free "$((db_port + 1))")"
proxy_port="$(next_free "$((http_port + 1))")"

# --- seed .env from the primary worktree's secrets (first run only) ----------
if [ ! -f .env ]; then
  if [ -f "$main_root/.env" ]; then
    cp "$main_root/.env" .env
    echo "[worktree-init] seeded .env from primary worktree ($main_root)"
  else
    cp .env.example .env
    echo "[worktree-init] primary .env not found; seeded from .env.example (edit secrets!)"
  fi
fi

# --- rewrite the managed block ----------------------------------------------
# Drop any prior managed block AND any standalone definitions of the keys we own,
# so we never depend on dotenv duplicate-key precedence (which differs between
# just's loader and dotenvy).
managed_keys="COMPOSE_PROJECT_NAME ZURFUR_DB_HOST_PORT ZURFUR_PROXY_HOST_PORT ZURFUR_HTTP_ADDR ZURFUR_PUBLIC_URL DATABASE_URL"

tmp="$(mktemp)"
awk -v keys="$managed_keys" '
  BEGIN { n = split(keys, a, " "); for (i = 1; i <= n; i++) owned[a[i]] = 1 }
  /^# >>> worktree isolation/ { inblock = 1; next }
  inblock && /^# <<< worktree isolation/ { inblock = 0; next }
  inblock { next }
  {
    line = $0
    if (line ~ /^[A-Za-z_][A-Za-z0-9_]*=/) {
      key = line; sub(/=.*/, "", key)
      if (key in owned) next
    }
    print
  }
' .env > "$tmp"
mv "$tmp" .env

# Trim a trailing blank line so the appended block reads cleanly, then append.
printf '%s\n' "$(cat .env)" > .env

cat >> .env <<EOF

# >>> worktree isolation ($slug) — managed by scripts/worktree-init.sh; do not edit
COMPOSE_PROJECT_NAME=$slug
ZURFUR_DB_HOST_PORT=$db_port
ZURFUR_PROXY_HOST_PORT=$proxy_port
ZURFUR_HTTP_ADDR=127.0.0.1:$http_port
ZURFUR_PUBLIC_URL=http://127.0.0.1:$http_port
DATABASE_URL=postgres://admin:password@localhost:$db_port/zurfur
# <<< worktree isolation
EOF

cat <<EOF
[worktree-init] isolated '$slug':
  compose project : $slug
  postgres        : localhost:$db_port  (DATABASE_URL updated)
  backend         : 127.0.0.1:$http_port
  proxy (profile) : 127.0.0.1:$proxy_port
Run \`just up\` / \`just dev\` here without colliding with other worktrees.
EOF
