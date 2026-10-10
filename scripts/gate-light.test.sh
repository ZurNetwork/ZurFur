#!/bin/sh
# Tests for scripts/gate-light.sh's plan (`--plan`, nothing runs) in a scratch
# git repo. Run directly, or via `just scripts-test`. Needs bash and git.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

failures=0
repo="$work/repo"

git_q() { git -C "$repo" -c user.email=t@example.com -c user.name=t "$@" >/dev/null 2>&1; }

# new_repo — a base commit with the classifier, one crate and a frontend,
# marked as origin/main, plus a PR branch to commit onto.
new_repo() {
    rm -rf "$repo"
    mkdir -p "$repo/scripts" "$repo/backend/crates/thing/src" "$repo/frontend/web"
    cp "$script_dir/ci-paths.sh" "$script_dir/gate-light.sh" "$repo/scripts/"
    printf '[package]\nname = "thing-pkg"\n' >"$repo/backend/crates/thing/Cargo.toml"
    echo "fn main() {}" >"$repo/backend/crates/thing/src/lib.rs"
    echo "{}" >"$repo/frontend/web/package.json"
    git -C "$repo" init -q -b main
    git_q add -A
    git_q commit -m base
    git_q update-ref refs/remotes/origin/main HEAD
}

# change <path> <content> — commits one file on top of the base.
change() {
    mkdir -p "$(dirname "$repo/$1")"
    echo "$2" >"$repo/$1"
    git_q add -A
    git_q commit -m change
}

# expect_plan <name> <fragment> [<fragment>…] — every fragment is in the plan.
expect_plan() {
    name="$1"
    shift
    out="$(cd "$repo" && bash scripts/gate-light.sh --plan 2>&1)" || {
        echo "FAIL $name: gate-light --plan exited non-zero"
        failures=$((failures + 1))
        return
    }
    for fragment in "$@"; do
        if ! printf '%s\n' "$out" | grep -qF -- "$fragment"; then
            echo "FAIL $name: plan lacks '$fragment'"
            printf '%s\n' "$out" | awk '{ print "  " $0 }'
            failures=$((failures + 1))
            return
        fi
    done
    echo "ok   $name"
}

new_repo
change docs/notes.md "hello"
expect_plan "docs only skips the tests and web" "no crate changed" "skipping the web checks"

new_repo
change backend/crates/thing/src/lib.rs "fn main() { }"
expect_plan "a crate change tests that crate" "cargo test --locked -p thing-pkg"

new_repo
change frontend/web/app.ts "export {}"
expect_plan "a frontend change runs the web checks" "yarn --cwd frontend/web run test"

new_repo
change Cargo.lock "# lock"
expect_plan "a path outside every crate tests the workspace" "cargo test --workspace --locked"

new_repo
change backend/crates/thing/src/lib.rs "fn main() { }"
printf '#!/bin/sh\nexit 1\n' >"$repo/scripts/ci-paths.sh"
expect_plan "a failing classifier runs everything" "classifier failed" "cargo test --workspace --locked" "yarn --cwd frontend/web run test"

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all gate-light tests passed"
