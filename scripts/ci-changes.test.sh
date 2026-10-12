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

# group_commit — commits whatever the case changed as one more squashed entry
# on top of the base, the way the merge queue builds its group branch, and
# leaves HEAD on it.
group_commit() {
    g checkout -q --detach HEAD
    g add -A
    g commit -m entry
    head_sha="$(git -C "$repo" rev-parse HEAD)"
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

# Merge-queue groups: everything runs; only `lock` follows the paths.
mg=GITHUB_EVENT_NAME=merge_group

new_base
g checkout -q main
echo "notes" >"$repo/docs.md"
group_commit
expect "a docs-only merge group still runs everything" "code=true lock=false " "$mg"
expect "a merge group with a missing head SHA runs everything" "code=true lock=true " "$mg" HEAD_SHA=
expect "a merge group whose HEAD isn't its head runs everything" "code=true lock=true " "$mg" HEAD_SHA="$base_sha"

new_base
g checkout -q main
echo "# lock" >"$repo/Cargo.lock"
group_commit
echo "notes" >"$repo/docs.md"
group_commit
expect "a Cargo.lock change from a PR ahead in the queue blocks on advisories" "code=true lock=true " "$mg"

new_base
g checkout -q main
printf '#!/bin/sh\nprintf "code=false\\nlock=false\\n"\n' >"$repo/scripts/ci-paths.sh"
echo "# lock" >"$repo/Cargo.lock"
group_commit
expect "a merge group's own classifier edit is ignored" "code=true lock=true " "$mg"

new_base
echo "side" >"$repo/side.md"
g add -A
g commit -m side
side_sha="$(git -C "$repo" rev-parse HEAD)"
g checkout -q main
echo "notes" >"$repo/docs.md"
group_commit
expect "a merge group whose base isn't an ancestor runs everything" "code=true lock=true " "$mg" BASE_SHA="$side_sha"

# A shallow clone that lacks the base (a checkout without full history).
new_base
g checkout -q main
echo "notes" >"$repo/docs.md"
group_commit
echo "more" >>"$repo/docs.md"
group_commit
full="$repo"
repo="$work/shallow"
rm -rf "$repo"
git clone -q --depth 1 "file://$full" "$repo" 2>/dev/null
g checkout -q --detach "$head_sha"
expect "a merge group checked out without its base runs everything" "code=true lock=true " "$mg"
repo="$full"

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all ci-changes tests passed"
