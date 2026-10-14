#!/bin/sh
# Pins the dev-profile lines in `just gate` and CI: the feature is in no
# default build, so dropping one of these lines would silently stop its lint
# or its tests. gate-light's own plan tests pin its copies. Run directly, or
# via `just scripts-test`.
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

if [ "$failures" -gt 0 ]; then
    echo "$failures dev-profile pin(s) missing"
    exit 1
fi
echo "all dev-profile pins hold"
