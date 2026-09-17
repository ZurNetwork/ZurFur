#!/bin/sh
# Tests for scripts/design-index-check.sh. Run directly, or via
# `just design-index-check-test`. POSIX sh + awk only — no `sed`.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/design-index-check.sh"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

failures=0

# expect_exit <name> <expected exit code> -- runs $under_test (with any
# trailing args after --) inside $repo, using whatever env vars the caller
# already exported.
expect_exit() {
    name="$1"
    want="$2"
    shift 2
    [ "${1:-}" = "--" ] && shift
    set +e
    (cd "$repo" && sh "$under_test" "$@") >"$work/out" 2>"$work/err"
    got=$?
    set -e
    if [ "$got" -ne "$want" ]; then
        echo "FAIL $name: expected exit $want, got $got"
        echo "  stdout: $(cat "$work/out")"
        echo "  stderr: $(cat "$work/err")"
        failures=$((failures + 1))
    else
        echo "ok   $name (exit $got)"
    fi
}

expect_contains() {
    name="$1"
    needle="$2"
    if ! grep -q "$needle" "$work/out" "$work/err" 2>/dev/null; then
        echo "FAIL $name: expected output to contain: $needle"
        failures=$((failures + 1))
    fi
}

# A fixture "corpus present" directory: the script's presence check is just
# `-f "$ZURFUR_DESIGN_DIR/Cargo.toml"`.
corpus="$work/corpus"
mkdir -p "$corpus"
touch "$corpus/Cargo.toml"

repo="$work/repo"
mkdir -p "$repo/docs"

# --- corpus absent + CI -> notice, exit 0 ---
export CI=true
export ZURFUR_DESIGN_DIR="$work/does-not-exist"
unset ZURFUR_DESIGN_SKIP DESIGN_INDEX_GENERATE_CMD 2>/dev/null || true
rm -f "$repo/docs/design-index.md"
expect_exit "absent + CI" 0
expect_contains "absent + CI" "::notice::"
unset CI

# --- corpus absent, no CI, no SKIP -> error, exit 1 ---
expect_exit "absent local" 1
expect_contains "absent local" "error: design corpus not present"

# --- corpus absent, ZURFUR_DESIGN_SKIP=1 -> notice, exit 0 ---
export ZURFUR_DESIGN_SKIP=1
expect_exit "absent local SKIP=1" 0
expect_contains "absent local SKIP=1" "notice:"
unset ZURFUR_DESIGN_SKIP

# From here on the corpus is present.
export ZURFUR_DESIGN_DIR="$corpus"

# --- corpus present, committed index missing -> warning, exit 0 ---
rm -f "$repo/docs/design-index.md"
echo "## entities/" >"$work/generated.md"
export DESIGN_INDEX_GENERATE_CMD="cat $work/generated.md"
expect_exit "missing committed index" 0
expect_contains "missing committed index" "warning:"

# --- index behind the corpus (additions only) -> warning, exit 0; --strict -> exit 1 ---
cat >"$repo/docs/design-index.md" <<'EOF'
## entities/
- 786439 current — User
EOF
cat >"$work/generated.md" <<'EOF'
## entities/
- 786439 current — User
- 1966081 current — Account
EOF
expect_exit "behind" 0
expect_contains "behind" "warning: docs/design-index.md is behind"
expect_exit "behind --strict" 1 -- --strict

# --- index contradicts the corpus (an id the corpus no longer has) -> error, exit 1 ---
cat >"$repo/docs/design-index.md" <<'EOF'
## entities/
- 786439 current — User
- 9999999 current — Ghost Page
EOF
cat >"$work/generated.md" <<'EOF'
## entities/
- 786439 current — User
EOF
expect_exit "contradiction: unknown id" 1
expect_contains "contradiction: unknown id" "is not in the corpus"

# --- index contradicts the corpus (a digit on a non-entry line) -> error, exit 1 ---
cat >"$repo/docs/design-index.md" <<'EOF'
## entities/
- 786439 current — User
This line should not have a stray 42 in it.
EOF
cat >"$work/generated.md" <<'EOF'
## entities/
- 786439 current — User
EOF
expect_exit "contradiction: stray digit" 1
expect_contains "contradiction: stray digit" "digit on a non-entry line"

# --- index contradicts the corpus (a non-generated header carrying a digit) -> error, exit 1 ---
cat >"$repo/docs/design-index.md" <<'EOF'
## entities 2/
- 786439 current — User
EOF
cat >"$work/generated.md" <<'EOF'
## entities/
- 786439 current — User
EOF
expect_exit "contradiction: bad header" 1
expect_contains "contradiction: bad header" "non-generated header"

if [ "$failures" -eq 0 ]; then
    echo "design-index-check.test.sh: all cases passed"
else
    echo "design-index-check.test.sh: $failures case(s) failed"
    exit 1
fi
