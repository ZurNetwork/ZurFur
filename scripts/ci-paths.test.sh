#!/bin/sh
# Tests for scripts/ci-paths.sh. Run directly, or via `just scripts-test`.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/ci-paths.sh"
failures=0

# expect <name> <expected output> <paths, one per line>
expect() {
    name="$1"
    want="$2"
    got="$(printf '%s' "$3" | sh "$under_test" | tr '\n' ' ')"
    if [ "$got" = "$want" ]; then
        echo "ok   $name"
    else
        echo "FAIL $name: expected '$want', got '$got'"
        failures=$((failures + 1))
    fi
}

expect "no paths fails closed" "code=true lock=true " ""
expect "markdown anywhere" "code=false lock=false " "README.md
backend/crates/domain/README.md
frontend/web/notes.md
"
expect "node files" "code=false lock=false " "NODE.json
backend/crates/api/NODE.json
NODE.toml
"
expect "document trees" "code=false lock=false " "docs/design-index.md
.understand/handoff.md
reports/x.html
forks/003/run.py
.claude/settings.json
"
expect "board config, scripts, Justfile" "code=false lock=false " ".agent-board.toml
scripts/jj-push.sh
scripts/hooks/pins.sh
Justfile
"
expect "a Rust file among docs" "code=true lock=false " "README.md
backend/crates/domain/src/lib.rs
"
expect "the workflow itself" "code=true lock=false " ".github/workflows/ci.yml"
expect "env example is read by a test" "code=true lock=false " ".env.example"
expect "the lockfile" "code=true lock=true " "Cargo.lock"
expect "a NODE.toml below the root is not listed" "code=true lock=false " "backend/NODE.toml"
expect "a lookalike prefix" "code=true lock=false " "docsx/file.rs"
expect "a quoted name git could not print plainly" "code=true lock=false " "\"docs/\\303\\251.md\""
expect "no trailing newline" "code=false lock=false " "README.md"

got="$(printf 'README.md\nCargo.lock\n' | sh "$under_test" --paths | tr '\n' '|')"
if [ "$got" = "docs README.md|code Cargo.lock|" ]; then
    echo "ok   --paths lists each path"
else
    echo "FAIL --paths lists each path: got '$got'"
    failures=$((failures + 1))
fi

if sh "$under_test" --bogus </dev/null >/dev/null 2>&1; then
    echo "FAIL an unknown flag should exit non-zero"
    failures=$((failures + 1))
else
    echo "ok   an unknown flag exits non-zero"
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all ci-paths tests passed"
