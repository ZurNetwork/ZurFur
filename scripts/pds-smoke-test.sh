#!/usr/bin/env bash
#
# pds-smoke-test.sh — scripted proof for ZMVP-102 ("The local PDS boots in
# the dev loop"). NOT a cargo test: the automated, per-test-throwaway PDS
# harness (testcontainers) is ZMVP-103's lane, deliberately — this proves the
# *dev loop* itself, at the shell/`just` level, end to end:
#
#   AC1  just up          -> PDS + PLC come up healthy alongside Postgres
#   AC2  pds-provision     -> creates a test account and signs in (session)
#   AC3  pds-reset         -> wipe is repeatable, not one-shot: re-provision
#                             after a wipe must mint a FRESH did:plc, proving
#                             the account state was actually cleared
#   AC4  isolation         -> nothing here can reach the public atproto
#                             network or canonical plc.directory
#   AC5  config coherence  -> every ZURFUR_PDS_*/ZURFUR_PLC_* key used by
#                             compose/just/scripts/dev.toml is documented in
#                             .env.example
#
# Run via `just pds-smoke` (loads .env through `just`'s dotenv-load). Leaves
# the stack down on exit either way.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; exit 1; }

cleanup() {
    echo "--- tearing down (docker compose down -v) ---"
    docker compose down -v > /dev/null 2>&1 || true
}
trap cleanup EXIT

echo "=== AC1: just up brings the PDS up healthy alongside Postgres ==="
just up
just _wait-for-db
just _wait-for-pds
pass "PDS + PLC + Postgres all report healthy"

echo "=== AC2: provision + sign-in returns a valid session ==="
first_session="$(just pds-provision)"
first_did="$(printf '%s' "$first_session" | python3 -c 'import json,sys; print(json.load(sys.stdin)["did"])')"
first_access="$(printf '%s' "$first_session" | python3 -c 'import json,sys; print(json.load(sys.stdin)["accessJwt"])')"
[ -n "$first_did" ] || fail "no did in the provisioned session"
[ -n "$first_access" ] || fail "no accessJwt in the provisioned session"
pass "provisioned did=$first_did with a session token"

echo "=== AC4: no request reaches the public atproto network / canonical plc.directory ==="
# Static: the network the pds/plc containers sit on is internal (no default
# route out — enforced by Docker/podman, not just config discipline), and the
# PDS's own config never names a public PLC or crawler.
docker compose config | grep -A2 '^networks:' | grep -q 'internal: true' \
    || fail "docker-compose network is not internal: true"
pass "compose network is internal: true (no default route out)"

resolved_env="$(docker compose config)"
if printf '%s' "$resolved_env" | grep -qi 'plc\.directory\|bsky\.network'; then
    fail "compose config references a public atproto/PLC host"
fi
pass "no public atproto/PLC hostname in resolved compose config"

# Runtime: confirm the pds container genuinely cannot resolve/reach a public
# host — not just that we didn't configure one.
if docker compose exec -T pds wget -T 3 -q -O- https://plc.directory/_health > /dev/null 2>&1; then
    fail "the pds container CAN reach https://plc.directory — isolation is broken"
fi
pass "pds container cannot reach https://plc.directory (network truly isolated)"

# Sibling guarantee (ZMVP-49): the app's own minter must still be non-submitting.
if [ "${ZURFUR_PLC_DIRECTORY_SUBMIT:-}" = "true" ]; then
    fail "ZURFUR_PLC_DIRECTORY_SUBMIT=true in this environment — refusing to smoke-test with it on"
fi
pass "ZURFUR_PLC_DIRECTORY_SUBMIT stays false"

echo "=== AC3: wipe is repeatable — re-provision after pds-reset mints a FRESH account ==="
just pds-reset
second_session="$(just pds-provision)"
second_did="$(printf '%s' "$second_session" | python3 -c 'import json,sys; print(json.load(sys.stdin)["did"])')"
[ -n "$second_did" ] || fail "no did in the re-provisioned session"
[ "$second_did" != "$first_did" ] || fail "re-provisioned did is IDENTICAL to before the wipe — state was not cleared"
pass "wipe cleared state: re-provisioned as a fresh did=$second_did (was $first_did)"

echo "=== AC5: every new ZURFUR_PDS_*/ZURFUR_PLC_* key is documented in .env.example ==="
used_keys="$(grep -rhoE 'ZURFUR_PDS_[A-Z_]+|ZURFUR_PLC_[A-Z_]+' \
    docker-compose.yml Justfile backend/config/dev.toml scripts/pds-provision.sh scripts/worktree-init.sh \
    | sort -u)"
missing=0
while IFS= read -r key; do
    [ -z "$key" ] && continue
    # A key is "documented" either as a real top-level assignment, or (for
    # the worktree-only host ports, same convention as the pre-existing
    # ZURFUR_DB_HOST_PORT/ZURFUR_PROXY_HOST_PORT) as the commented example in
    # the "Worktree isolation" block.
    if ! grep -qE "^#?[[:space:]]*${key}=" .env.example; then
        echo "  FAIL: $key is used but not documented in .env.example" >&2
        missing=1
    fi
done <<< "$used_keys"
[ "$missing" -eq 0 ] || fail "one or more keys undocumented (see above)"
pass "every referenced ZURFUR_PDS_*/ZURFUR_PLC_* key is documented in .env.example"

echo ""
echo "=== ALL CHECKS PASSED ==="
