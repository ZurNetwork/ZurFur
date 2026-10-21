#!/bin/sh
# Pins the dev-profile lines in `just gate` and CI: the feature is in no
# default build, so dropping one of these lines would silently stop its lint
# or its tests. gate-light's own plan tests pin its copies. Also pins its
# Caddy: out of plain `just up`, and on loopback only, since everyone it
# serves signs in as a fixture user. Run directly, or via `just scripts-test`.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
failures=0

clippy='CARGO_TARGET_DIR=target/dev-profile cargo clippy -p api --all-targets --locked --features dev-profile -- -D warnings'
tests='cargo test -p api --locked --features dev-profile --lib --test dev_profile'

# The body of the Justfile's `gate` recipe: its indented lines up to the next recipe.
gate_body="$(awk '
    /^gate:/ { inside = 1; next }
    inside && /^[^[:space:]]/ { inside = 0 }
    inside { print }
' "$root/Justfile")"

# expect <name> <haystack> <needle>
expect() {
    if printf '%s\n' "$2" | grep -qF -- "$3"; then
        echo "ok   $1"
    else
        echo "FAIL $1: lacks '$3'"
        failures=$((failures + 1))
    fi
}

expect "just gate lints the dev profile in its own target dir" "$gate_body" "$clippy"
expect "just gate tests the dev profile in its own target dir" "$gate_body" "CARGO_TARGET_DIR=target/dev-profile INSTA_UPDATE=no $tests"

ci="$(cat "$root/.github/workflows/ci.yml")"
expect "CI lints the dev profile" "$ci" "- run: $clippy"
expect "CI tests the dev profile" "$ci" "- run: CARGO_TARGET_DIR=target/dev-profile $tests"

# The body of docker-compose.yml's `caddy-dev-profile` service: its lines up to the next service.
dev_caddy="$(awk '
    /^  caddy-dev-profile:/ { inside = 1; next }
    inside && /^  [^[:space:]]/ { inside = 0 }
    inside { print }
' "$root/docker-compose.yml")"
# expect_line <name> <haystack> <line>: one whole line, indentation aside, is <line>.
expect_line() {
    if printf '%s\n' "$2" | awk -v want="$3" '
        { sub(/^[[:space:]]+/, ""); sub(/[[:space:]]+$/, "") }
        $0 == want { found = 1 }
        END { exit !found }
    '; then
        echo "ok   $1"
    else
        echo "FAIL $1: no line '$3'"
        failures=$((failures + 1))
    fi
}

expect_line "the dev profile's Caddy waits for --profile dev-profile" "$dev_caddy" "profiles: [dev-profile]"
expect_line "the dev profile's Caddy serves the shared Caddyfile" "$dev_caddy" "- ./caddy/Caddyfile:/etc/caddy/Caddyfile:ro"
expect_line "the Caddyfile binds loopback only" "$(cat "$root/caddy/Caddyfile")" "bind 127.0.0.1"

# The body of the Justfile's `_dev-profile-web` recipe. Its vite run is the one
# place ZURFUR_WEB_STACK=dev-profile is set: the web tells the dev profile apart
# by it, and no spec sees a recipe.
dev_profile_web="$(awk '
    /^_dev-profile-web / { inside = 1; next }
    inside && /^[^[:space:]]/ { inside = 0 }
    inside { print }
' "$root/Justfile")"
expect "_dev-profile-web marks its web run as the dev profile" "$(printf '%s\n' "$dev_profile_web" | grep -F 'yarn dev')" "ZURFUR_WEB_STACK=dev-profile yarn dev"

if [ "$failures" -gt 0 ]; then
    echo "$failures dev-profile pin(s) missing"
    exit 1
fi
echo "all dev-profile pins hold"
