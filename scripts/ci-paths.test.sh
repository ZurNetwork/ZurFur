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

expect "no paths fails closed" "code=true lock=true npmlock=true " ""
expect "markdown outside the code trees" "code=false lock=false npmlock=false " "README.md
.github/copilot-instructions.md
docs/notes.md
"
expect "markdown under frontend is read by prettier" "code=true lock=false npmlock=false " "frontend/web/README.md"
expect "markdown under backend counts as code" "code=true lock=false npmlock=false " "backend/crates/domain/README.md"
expect "markdown under contract counts as code" "code=true lock=false npmlock=false " "contract/VERSIONING.md"
expect "a NODE.inf.json under frontend stays docs-only" "code=false lock=false npmlock=false " "frontend/web/NODE.inf.json"
expect "a NODE.inf.json in a crate's queries/ is code (sqlx-rust-codegen)" "code=true lock=false npmlock=false " "backend/crates/adapter-pg/queries/account/NODE.inf.json"
expect "a NODE.inf.json in the api's generated/ is code (contract_current)" "code=true lock=false npmlock=false " "backend/crates/api/src/generated/NODE.inf.json"
expect "a NODE.inf.json in the web's generated/ is code (contract regeneration)" "code=true lock=false npmlock=false " "frontend/web/src/lib/server/api/generated/NODE.inf.json"
expect "node files" "code=false lock=false npmlock=false " "NODE.inf.json
backend/crates/api/NODE.inf.json
NODE.refs.json
NODE.toml
"
expect "NODE.refs.json alone is docs-only" "code=false lock=false npmlock=false " "NODE.refs.json"
expect "a stray NODE.json at the root is code" "code=true lock=false npmlock=false " "NODE.json"
expect "a stray NODE.json in a code tree is code" "code=true lock=false npmlock=false " "backend/x/NODE.json"
expect "document trees" "code=false lock=false npmlock=false " "docs/design-index.md
reports/x.html
forks/003/run.py
.claude/settings.json
"
expect "scripts, Justfile" "code=false lock=false npmlock=false " "scripts/jj-push.sh
scripts/hooks/pins.sh
Justfile
"
expect "the screenshot script is run by the ui job" "code=true lock=false npmlock=false " "scripts/gallery-shots.sh"
expect "the screenshot script's tests are not" "code=false lock=false npmlock=false " "scripts/gallery-shots.test.sh
scripts/gallery-image-pins.test.sh
"
expect "a Rust file among docs" "code=true lock=false npmlock=false " "README.md
backend/crates/domain/src/lib.rs
"
expect "the workflow itself" "code=true lock=false npmlock=false " ".github/workflows/ci.yml"
expect "env example is read by a test" "code=true lock=false npmlock=false " ".env.example"
expect "Cargo.lock blocks cargo advisories, not npm ones" "code=true lock=true npmlock=false " "Cargo.lock"
expect "the frontend lockfile" "code=true lock=false npmlock=true " "frontend/yarn.lock"
expect "the lexicons lockfile" "code=true lock=false npmlock=true " "lexicons/package-lock.json"
expect "a root yarn.lock" "code=true lock=false npmlock=true " "yarn.lock"
expect "a root package-lock.json" "code=true lock=false npmlock=true " "package-lock.json"
expect "the frontend ignore file" "code=true lock=false npmlock=true " "osv-scanner.frontend.toml"
expect "the lexicons ignore file" "code=true lock=false npmlock=true " "osv-scanner.lexicons.toml"
expect "an npm lockfile among docs" "code=true lock=false npmlock=true " "README.md
frontend/yarn.lock
"
expect "the root licence list" "code=true lock=false npmlock=true " "npm-licences.json"
expect "a licence list below the root is not one" "code=true lock=false npmlock=false " "frontend/npm-licences.json"
expect "an ignore file below the root is not one" "code=true lock=false npmlock=false " "lexicons/osv-scanner.lexicons.toml"
expect "README.md is not an npm lockfile" "code=false lock=false npmlock=false " "README.md"
expect "the Justfile is not an npm lockfile" "code=false lock=false npmlock=false " "Justfile"
expect "a NODE.toml below the root is not listed" "code=true lock=false npmlock=false " "backend/NODE.toml"
expect "a lookalike prefix" "code=true lock=false npmlock=false " "docsx/file.rs"
expect "a quoted name git could not print plainly" "code=true lock=false npmlock=false " "\"docs/\\303\\251.md\""
expect "no trailing newline" "code=false lock=false npmlock=false " "README.md"

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
