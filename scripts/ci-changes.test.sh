#!/bin/sh
# Tests for scripts/ci-changes.sh (the body of CI's `changes` job) against
# scratch git repos with real test-merge commits. Run directly, or via
# `just scripts-test`. Needs git.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/ci-changes.sh"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

failures=0
repo="$work/repo"

g() { git -C "$repo" -c user.email=t@example.com -c user.name=t "$@" >/dev/null 2>&1; }

# new_base [no-classifier] — a base commit on main, then a `pr` branch.
new_base() {
    rm -rf "$repo"
    mkdir -p "$repo/scripts" "$repo/backend/crates/thing/src" "$repo/frontend/web"
    if [ "${1:-}" != no-classifier ]; then
        cp "$script_dir/ci-paths.sh" "$repo/scripts/"
    fi
    echo "fn main() {}" >"$repo/backend/crates/thing/src/lib.rs"
    echo "# web" >"$repo/frontend/web/README.md"
    echo "# repo" >"$repo/README.md"
    git -C "$repo" init -q -b main
    g add -A
    g commit -m base
    base_sha="$(git -C "$repo" rev-parse HEAD)"
    g checkout -b pr
}

# pr_commit — commits whatever the case changed on the pr branch, then builds
# GitHub's test merge (base as first parent, PR head as second) and checks it out.
pr_commit() {
    g add -A
    g commit -m change
    head_sha="$(git -C "$repo" rev-parse HEAD)"
    g checkout -q --detach "$base_sha"
    g merge --no-ff -m merge pr
}

# expect <name> <expected stdout> [env assignments…]
expect() {
    name="$1"
    want="$2"
    shift 2
    got="$(cd "$repo" && env GITHUB_EVENT_NAME=pull_request BASE_SHA="$base_sha" HEAD_SHA="$head_sha" "$@" sh "$under_test" 2>/dev/null | tr '\n' ' ')"
    if [ "$got" = "$want" ]; then
        echo "ok   $name"
    else
        echo "FAIL $name: expected '$want', got '$got'"
        failures=$((failures + 1))
    fi
}

new_base
echo "notes" >"$repo/docs.md"
pr_commit
expect "a docs-only PR skips" "code=false lock=false "
expect "a push runs everything" "code=true lock=true " GITHUB_EVENT_NAME=push
expect "a missing base SHA runs everything" "code=true lock=true " BASE_SHA=
expect "merge parents that aren't base and head run everything" "code=true lock=true " BASE_SHA="$head_sha"

new_base
echo "* star list" >>"$repo/frontend/web/README.md"
pr_commit
expect "Markdown under frontend runs everything" "code=true lock=false "

new_base
mkdir -p "$repo/docs"
git -C "$repo" mv backend/crates/thing/src/lib.rs docs/lib.md
pr_commit
expect "a crate file moved into docs runs everything" "code=true lock=false "

new_base
printf '#!/bin/sh\nprintf "code=false\\nlock=false\\n"\n' >"$repo/scripts/ci-paths.sh"
echo "fn main() { }" >"$repo/backend/crates/thing/src/lib.rs"
pr_commit
expect "the PR's own classifier edit is ignored" "code=true lock=false "

new_base
echo "# lock" >"$repo/Cargo.lock"
pr_commit
expect "a Cargo.lock change blocks on advisories" "code=true lock=true "

new_base no-classifier
echo "notes" >"$repo/docs.md"
pr_commit
expect "a base without the classifier runs everything" "code=true lock=true "

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all ci-changes tests passed"
