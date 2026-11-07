#!/bin/sh
# Tests for the zaf step actions in .zaf/actions: `push`'s ancestry check
# (a rewritten pushed commit is refused; a forward commit and a base merge
# push), and the task-branch check of `commit`, `push` and `pr-open` (main and
# the integration branch are refused before anything runs). The push fixtures
# use a scratch jj repo and a local bare origin, never the real one. Run
# directly, or via `just scripts-test`. Without jj or the build slot (CI's
# `hooks` job) the push fixtures print a skip line.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
actions="$repo_root/.zaf/actions"
failures=0

ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1: $2"
    failures=$((failures + 1))
}

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# expect <name> <want exit> <text the output must hold> <action> <args...>:
# runs the action with ZAF_ROOT set to the scratch root.
expect() {
    name="$1" want="$2" text="$3" action="$4"
    shift 4
    got=0
    out="$(ZAF_ROOT="$tmp/root" sh "$actions/$action" "$@" 2>&1)" || got=$?
    if [ "$got" != "$want" ]; then
        fail "$name" "exit $got, wanted $want: $out"
        return
    fi
    case $out in
        *"$text"*) ok "$name" ;;
        *) fail "$name" "output lacks \"$text\": $out" ;;
    esac
}

# The task-branch check: main and the integration branch are not slices, and
# neither is the base the run names.
mkdir -p "$tmp/root"
export ZAF_VERSION=0.3
for branch in main feature/release-0.3; do
    expect "push refuses $branch" 1 "is not a task branch" push "$branch"
    expect "commit refuses $branch" 1 "is not a task branch" commit "$branch" message
    expect "pr-open refuses $branch" 1 "is not a task branch" pr-open "$branch" main title body
done
expect "pr-open refuses its own base" 1 "is the base" pr-open feature/release-0-3-1 feature/release-0-3-1 title body
export ZAF_BASE=feature/release-0-3-1
expect "push refuses the base" 1 "is the base" push feature/release-0-3-1
expect "commit refuses the base" 1 "is the base" commit feature/release-0-3-1 message
unset ZAF_BASE ZAF_VERSION

slot=/home/zuri/.cache/zurfur-locks/build-slot.sh
if ! command -v jj >/dev/null 2>&1 || [ ! -x "$slot" ]; then
    echo "skip (no jj or no build slot): the push fixtures"
    [ "$failures" -eq 0 ] || exit 1
    exit 0
fi

# One scratch repo, isolated as scripts/jj-push.test.sh isolates it. The root
# commits the two scripts push runs from the root (no .rs/.toml, so the cargo
# gate skips itself) and ignores .zaf/, where the slice's workspace lives.
printf '[user]\nname = "fixture"\nemail = "fixture@example.invalid"\n' >"$tmp/config.toml"
export JJ_CONFIG="$tmp/config.toml" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
root="$tmp/root"
origin="$tmp/origin.git"
slice=feature/release-0.3-1-2
dir="$root/.zaf/workspaces/$slice"
git init -q --bare "$origin"
jj git init "$root" >/dev/null 2>&1
(
    cd "$root"
    jj git remote add origin "$origin"
    mkdir scripts
    cp "$script_dir/jj-push.sh" "$script_dir/private-papers-check.sh" scripts/
    echo .zaf/ >.gitignore
    jj commit -m "base" >/dev/null 2>&1
    jj bookmark create base -r @- >/dev/null 2>&1
    mkdir -p "$(dirname "$dir")"
    jj workspace add --name "$slice" -r base "$dir" >/dev/null 2>&1
)

# in_slice <message>: commit a change in the slice's workspace.
in_slice() {
    (cd "$dir" && echo "$1" >>work.txt && jj commit -m "$1" >/dev/null 2>&1)
}
# on_origin: the slice's commit on origin, or nothing.
on_origin() {
    git --git-dir="$origin" rev-parse --verify -q "refs/heads/$slice" || true
}
# slice_head: the slice workspace's latest commit.
slice_head() {
    (cd "$dir" && jj log --no-graph -r @- -T commit_id)
}

in_slice "first"
expect "push sends a first push" 0 "" push "$slice"
[ "$(on_origin)" = "$(slice_head)" ] && ok "origin holds the first push" || fail "origin holds the first push" "origin at $(on_origin)"

in_slice "second"
expect "push passes a forward commit" 0 "" push "$slice"
[ "$(on_origin)" = "$(slice_head)" ] && ok "origin holds the forward commit" || fail "origin holds the forward commit" "origin at $(on_origin)"

# A base merge: the base moves on in the root, and the slice merges it in.
(cd "$root" && jj new base -m "base moves" >/dev/null 2>&1 && echo b >base.txt && jj commit -m "base moves" >/dev/null 2>&1 && jj bookmark set base -r @- >/dev/null 2>&1)
(cd "$dir" && jj new @- base -m "merge base" >/dev/null 2>&1 && jj new >/dev/null 2>&1)
expect "push passes a base merge" 0 "" push "$slice"
[ "$(on_origin)" = "$(slice_head)" ] && ok "origin holds the base merge" || fail "origin holds the base merge" "origin at $(on_origin)"

# A rewritten pushed commit: origin's bookmark is no longer an ancestor.
before="$(on_origin)"
(cd "$dir" && jj describe -r @- -m "merge base, rewritten" >/dev/null 2>&1)
expect "push refuses a rewritten pushed commit" 1 "rewrote a pushed commit" push "$slice"
[ "$(on_origin)" = "$before" ] && ok "origin keeps the pushed commit" || fail "origin keeps the pushed commit" "origin moved to $(on_origin)"

[ "$failures" -eq 0 ] || exit 1
