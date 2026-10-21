#!/usr/bin/env bash
#
# workspace-init.sh — give the current jj workspace its own isolated runtime.
#
# Every Zurfur integration test already spins up a throwaway PostgreSQL container
# on a RANDOM host port (testcontainers) and binds the HTTP server to :0, so the
# test suite is collision-free across workspaces out of the box. The thing that is
# NOT isolated is the *manual* dev environment: `just up` / `just dev` use the
# fixed host ports 5432 (db) and 8080 (Caddy public origin), and a single shared
# DB volume.
# Two workspaces running it at once would fight over those ports and corrupt each
# other's schema.
#
# This script closes that gap. It writes a managed block into THIS workspace's
# `.env` that pins:
#   * COMPOSE_PROJECT_NAME         — namespaces containers + the pg_data/pds_data volumes
#   * ZURFUR_DB_HOST_PORT          — the host port docker-compose maps Postgres to
#   * ZURFUR_PROXY_HOST_PORT       — Caddy's public-origin listen port
#   * ZURFUR_WEB_PORT              — the SvelteKit vite dev server port
#   * ZURFUR_API_UPSTREAM          — the internal axum origin Caddy + SSR proxy to
#   * ZURFUR_PDS_HOST_PORT         — the host port docker-compose maps the dev PDS to
#   * ZURFUR_PDS_ENDPOINT          — kept coherent with the PDS port above
#   * ZURFUR_PLC_HOST_PORT         — the host port docker-compose maps the local PLC to
#   * ZURFUR_PLC_DIRECTORY_ENDPOINT — kept coherent with the PLC port above
#   * ZURFUR_HTTP_ADDR             — where axum binds INTERNALLY (env wins over dev.toml)
#   * ZURFUR_PUBLIC_URL            — the BROWSER origin = Caddy's proxy port, so OAuth/CSRF stay coherent
#   * DATABASE_URL                 — points sqlx/the app at this workspace's DB port
#
# Ports are derived from the workspace's directory name, so they are stable across
# re-runs (a given workspace always gets the same ports) yet distinct per workspace.
# Secrets (JWT/OAuth/PDS keys) are copied once from the primary checkout's `.env`.
#
# The primary checkout is jj's `default` workspace; without jj (a plain git clone
# with `git worktree add`), it is the first entry of `git worktree list`.
#
# Idempotent: re-running rewrites the managed block in place and leaves the rest
# of `.env` — including your secrets — untouched. The primary checkout keeps the
# defaults (5432/8080); there is no reason to run this there.

set -euo pipefail

# The primary checkout holds the canonical `.env` with the real secrets; we seed
# from it so a fresh workspace (where `.env` is gitignored and therefore absent)
# still has working keys.
if root="$(jj workspace root --ignore-working-copy 2>/dev/null)"; then
  main_root="$(jj workspace root --ignore-working-copy --name default 2>/dev/null || true)"
elif root="$(git rev-parse --show-toplevel 2>/dev/null)"; then
  main_root="$(git worktree list --porcelain | awk '/^worktree /{print $2; exit}')"
else
  echo "[workspace-init] not inside a jj workspace or a git checkout" >&2
  exit 1
fi
cd "$root"

slug="$(basename "$root")"

if [ -z "$main_root" ]; then
  echo "[workspace-init] no primary checkout found (jj has no workspace named 'default')" >&2
  exit 1
fi

if [ "$root" = "$main_root" ]; then
  cat >&2 <<EOF
refusing to isolate the PRIMARY checkout ($slug).
The primary checkout keeps the default ports (db 5432, http 8080). Run this from
a secondary workspace created by \`jj workspace add\`.
EOF
  exit 1
fi

# --- pick six stable, free host ports ---------------------------------------
# Base is a hash of the slug folded into 20000–38999, leaving room for +1..+5.
# We then walk forward past any port that currently has a listener, so a re-run
# while the stack is up still lands on this workspace's own (already-bound) ports
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
pds_port="$(next_free "$((proxy_port + 1))")"
plc_port="$(next_free "$((pds_port + 1))")"
web_port="$(next_free "$((plc_port + 1))")"

# --- seed .env from the primary checkout's secrets (first run only) ----------
if [ ! -f .env ]; then
  if [ -f "$main_root/.env" ]; then
    cp "$main_root/.env" .env
    echo "[workspace-init] seeded .env from the primary checkout ($main_root)"
  else
    cp .env.example .env
    echo "[workspace-init] primary .env not found; seeded from .env.example (edit secrets!)"
  fi
fi

# --- seed missing managed-secret keys on re-run (without printing values) ----
# If a new managed secret was added to the project after this workspace's
# initial .env (e.g., new PDS/PLC keys), re-running this script should
# seed it without overwriting existing keys or printing their values.
managed_secrets="ZURFUR_PDS_JWT_SECRET ZURFUR_PDS_ADMIN_PASSWORD ZURFUR_PDS_PLC_ROTATION_KEY"
for secret_key in $managed_secrets; do
  if ! grep -q "^$secret_key=" .env; then
    # Try to seed from the primary checkout's .env first, then fall back to .env.example
    if [ -f "$main_root/.env" ] && grep -q "^$secret_key=" "$main_root/.env"; then
      grep "^$secret_key=" "$main_root/.env" >> .env
      echo "[workspace-init] seeded missing $secret_key from the primary checkout"
    elif grep -q "^$secret_key=" .env.example; then
      grep "^$secret_key=" .env.example >> .env
      echo "[workspace-init] seeded missing $secret_key from .env.example"
    fi
  fi
done

# --- rewrite the managed block ----------------------------------------------
# Drop any prior managed block (the older `worktree isolation` marker too) AND
# any standalone definitions of the keys we own, so we never depend on dotenv
# duplicate-key precedence (which differs between just's loader and dotenvy).
managed_keys="COMPOSE_PROJECT_NAME ZURFUR_DB_HOST_PORT ZURFUR_PROXY_HOST_PORT ZURFUR_WEB_PORT ZURFUR_API_UPSTREAM ZURFUR_PDS_HOST_PORT ZURFUR_PDS_ENDPOINT ZURFUR_PLC_HOST_PORT ZURFUR_PLC_DIRECTORY_ENDPOINT ZURFUR_HTTP_ADDR ZURFUR_PUBLIC_URL DATABASE_URL"

tmp="$(mktemp)"
awk -v keys="$managed_keys" '
  BEGIN { n = split(keys, a, " "); for (i = 1; i <= n; i++) owned[a[i]] = 1 }
  /^# >>> (workspace|worktree) isolation/ { inblock = 1; next }
  inblock && /^# <<< (workspace|worktree) isolation/ { inblock = 0; next }
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

# >>> workspace isolation ($slug) — managed by scripts/workspace-init.sh; do not edit
COMPOSE_PROJECT_NAME=$slug
ZURFUR_DB_HOST_PORT=$db_port
ZURFUR_PROXY_HOST_PORT=$proxy_port
ZURFUR_WEB_PORT=$web_port
ZURFUR_API_UPSTREAM=http://127.0.0.1:$http_port
ZURFUR_PDS_HOST_PORT=$pds_port
ZURFUR_PDS_ENDPOINT=http://localhost:$pds_port
ZURFUR_PLC_HOST_PORT=$plc_port
ZURFUR_PLC_DIRECTORY_ENDPOINT=http://localhost:$plc_port
ZURFUR_HTTP_ADDR=127.0.0.1:$http_port
ZURFUR_PUBLIC_URL=http://127.0.0.1:$proxy_port
DATABASE_URL=postgres://admin:password@localhost:$db_port/zurfur
# <<< workspace isolation
EOF

cat <<EOF
[workspace-init] isolated '$slug':
  compose project : $slug
  postgres        : localhost:$db_port  (DATABASE_URL updated)
  public origin   : 127.0.0.1:$proxy_port  (Caddy = ZURFUR_PUBLIC_URL)
  axum (internal) : 127.0.0.1:$http_port  (ZURFUR_API_UPSTREAM, behind Caddy)
  web (SvelteKit) : 127.0.0.1:$web_port  (ZURFUR_WEB_PORT, behind Caddy)
  dev PDS         : localhost:$pds_port  (ZURFUR_PDS_ENDPOINT updated)
  local PLC       : localhost:$plc_port  (ZURFUR_PLC_DIRECTORY_ENDPOINT updated)
Run \`just up\` / \`just dev\` here without colliding with other workspaces.
EOF
