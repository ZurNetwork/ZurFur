#!/usr/bin/env bash
#
# web-smoke.sh — scripted proof for ZMVP-150 ("one origin, one app"). NOT a
# cargo test: like `pds-smoke`, this proves the *dev loop* itself at the
# shell/`just` level. It asserts, through the Caddy public origin, that the
# one-origin split routes exactly as designed:
#
#   /api/health           -> axum, /api stripped        (AC2 + AC3 upstream)
#   /                      -> SvelteKit (HTML)           (AC2 catch-all)
#   /.well-known/atproto-did  -> axum verbatim          (AC2 carve-out)
#   /signin-callback       -> axum verbatim             (AC2 carve-out, OAuth)
#   /apifoo                -> SvelteKit, NOT the backend (AC2 boundary: /api is
#                                                         a prefix, not a stem)
#
# Unlike `pds-smoke`, this does NOT boot anything: axum (`cargo run`) and the
# vite dev server (`yarn dev`) run on the HOST, not in Docker, so there is
# nothing for a script to `docker compose up`. Bring the stack up first
# (`just dev`, or the three legs by hand), then run `just web-smoke`. The origin
# is read from ZURFUR_PUBLIC_URL, so it targets whichever worktree you run it in.
set -euo pipefail

origin="${ZURFUR_PUBLIC_URL:-http://127.0.0.1:8080}"

pass() { echo "  PASS: $1"; }
fail() { echo "  FAIL: $1" >&2; exit 1; }

# GET $1; leaves the status code in REPLY_CODE and the body in REPLY_BODY.
http_get() {
    local path="$1"
    local tmp
    tmp="$(mktemp)"
    REPLY_CODE="$(curl -sS -o "$tmp" -w '%{http_code}' "$origin$path" || true)"
    REPLY_BODY="$(cat "$tmp")"
    rm -f "$tmp"
}

echo "=== web-smoke against $origin ==="

# Precondition: the origin must be reachable. If it isn't, say so plainly rather
# than emit a wall of confusing per-check failures.
if ! curl -sS -o /dev/null --max-time 5 "$origin/" 2>/dev/null; then
    fail "Caddy origin $origin is unreachable — start the stack first (\`just dev\`), then re-run \`just web-smoke\`."
fi

echo "--- AC2/AC3: /api/* -> axum with the /api prefix stripped ---"
http_get "/api/health"
[ "$REPLY_CODE" = "200" ] || fail "/api/health returned $REPLY_CODE, expected 200 (is the backend up?)"
# axum's health body is JSON carrying a status + database field; a SvelteKit
# 404 would be HTML. Assert the axum shape.
printf '%s' "$REPLY_BODY" | grep -q '"status"' \
    || fail "/api/health body is not axum health JSON: $REPLY_BODY"
printf '%s' "$REPLY_BODY" | grep -q '"database"' \
    || fail "/api/health body is missing the database field: $REPLY_BODY"
pass "/api/health -> 200 axum health JSON (/api stripped to /health): $REPLY_BODY"

echo "--- AC2: / -> the SvelteKit app (HTML) ---"
http_get "/"
[ "$REPLY_CODE" = "200" ] || fail "/ returned $REPLY_CODE, expected 200 (is the vite dev server up?)"
printf '%s' "$REPLY_BODY" | grep -qi '<!doctype html' \
    || fail "/ body is not an HTML document (SvelteKit should render one): ${REPLY_BODY:0:200}"
pass "/ -> 200 SvelteKit HTML document"

echo "--- AC2: /.well-known/atproto-did -> axum verbatim (NOT SvelteKit) ---"
http_get "/.well-known/atproto-did"
# The resolver only answers for a *.zurfur.app Host; via the loopback origin the
# Host is not ours, so axum answers 404 with an EMPTY body. A misroute to
# SvelteKit would instead return its HTML 404 page — the discriminator is the
# absence of an HTML document, proving axum (not SvelteKit) handled the path.
[ "$REPLY_CODE" = "404" ] || fail "/.well-known/atproto-did returned $REPLY_CODE, expected axum's 404"
if printf '%s' "$REPLY_BODY" | grep -qi '<!doctype html\|<html'; then
    fail "/.well-known/atproto-did returned an HTML document — it hit SvelteKit, not the axum resolver"
fi
pass "/.well-known/atproto-did -> axum 404 (non-HTML body): reached the resolver, not SvelteKit"

echo "--- AC2: /signin-callback -> axum verbatim (the OAuth callback path) ---"
http_get "/signin-callback"
# The callback with no query params is a 303 to /login?error=invalid_callback
# from axum (ZMVP-151: callback failures redirect to the SvelteKit login page).
# SvelteKit has no such route and would 404. The 303 proves axum handled it
# verbatim (no /api strip, no SvelteKit). curl does not follow it, so the code
# and Location are the callback handler's own.
[ "$REPLY_CODE" = "303" ] \
    || fail "/signin-callback returned $REPLY_CODE, expected axum's 303 (SvelteKit would 404)"
pass "/signin-callback -> axum 303 (reached the OAuth callback handler verbatim)"

echo "--- AC2: /apifoo does NOT reach the backend (/api is a prefix, not a stem) ---"
http_get "/apifoo"
# /apifoo must fall through to SvelteKit: it does not match /api/* nor a bare
# /api. SvelteKit renders a full HTML document (its 404 page); axum would have
# returned an empty-bodied 404. The presence of an HTML document proves the
# backend was NOT reached.
printf '%s' "$REPLY_BODY" | grep -qi '<!doctype html\|<html' \
    || fail "/apifoo did not return a SvelteKit HTML document (code $REPLY_CODE) — it may have reached the backend"
pass "/apifoo -> SvelteKit HTML (code $REPLY_CODE): the backend was not reached"

echo ""
echo "=== ALL CHECKS PASSED ==="
