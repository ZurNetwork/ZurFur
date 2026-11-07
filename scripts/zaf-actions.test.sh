#!/bin/sh
# Tests for the zaf step actions in .zaf/actions: `push`'s ancestry check
# (a rewritten pushed commit is refused; a forward commit and a base merge
# push), and the task-branch check of `commit`, `push`, `pr-open`,
# `workspace-open`, `workspace-drop` and `merge-base` (main and the integration
# branch are refused before anything runs). The push fixtures
# use a scratch jj repo and a local bare origin, never the real one, and a
# copy of `push` whose build slot is a stub, so a gate that already holds a
# slot never waits on another. Run directly, or via `just scripts-test`.
# Without jj (CI's `hooks` job) the push fixtures print a skip line.
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

# A gh that always fails comes first on PATH, so no action under test can
# reach the real repository.
mkdir -p "$tmp/bin"
printf '#!/bin/sh\necho "gh stub: $*" >&2\nexit 1\n' >"$tmp/bin/gh"
chmod +x "$tmp/bin/gh"
PATH="$tmp/bin:$PATH"
export PATH

# expect <name> <want exit> <text the output must hold> <action> <args...>:
# runs the action from $actions with ZAF_ROOT set to the scratch root.
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
expect "workspace-open refuses main" 1 "is not a task branch" workspace-open main feature/release-0.3
expect "workspace-drop refuses the integration branch" 1 "is not a task branch" workspace-drop feature/release-0.3
expect "merge-base refuses main" 2 "is not a task branch" merge-base main feature/release-0.3
expect "pr-open refuses its own base" 1 "is the base" pr-open feature/release-0-3-1 feature/release-0-3-1 title body
export ZAF_BASE=feature/release-0.3
expect "pr-open refuses a slice of another release" 1 "is not a task branch of release 0.3" pr-open feature/release-0.4-1-2 feature/release-0.3 title body
expect "pr-open refuses a base other than ZAF_BASE" 1 "is not ZAF_BASE" pr-open feature/release-0.3-1-2 main title body
expect "pr-open reaches only the stub gh" 2 "gh stub: pr list" pr-open feature/release-0.3-1-2 feature/release-0.3 title body
export ZAF_BASE=feature/release-0-3-1
expect "push refuses the base" 1 "is the base" push feature/release-0-3-1
expect "commit refuses the base" 1 "is the base" commit feature/release-0-3-1 message
unset ZAF_BASE ZAF_VERSION

if ! command -v jj >/dev/null 2>&1; then
    echo "skip (no jj): the push fixtures"
    [ "$failures" -eq 0 ] || exit 1
    exit 0
fi

# The fixtures run copies of the actions, `push`'s with a build slot that is a
# stub running its command at once: the real slot would wait on the slot the
# gate running this test already holds.
slot_line='slot=/home/zuri/.cache/zurfur-locks/build-slot.sh'
mkdir -p "$tmp/actions"
cp "$actions"/* "$tmp/actions/"
printf '#!/bin/sh\nexec "$@"\n' >"$tmp/slot"
chmod +x "$tmp/slot"
awk -v from="$slot_line" -v to="slot=$tmp/slot" '$0 == from { print to; found = 1; next } { print } END { exit !found }' \
    "$actions/push" >"$tmp/actions/push" || {
    fail "push's copy" "push has no line $slot_line to point at the stub"
    exit 1
}
actions="$tmp/actions"

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

# workspace-open hands back a directory only when jj holds it under the
# slice's name: not when jj holds the slice elsewhere and another workspace
# sits at its directory, and not when another workspace alone sits there.
expect "workspace-open hands back the slice's own workspace" 0 "$dir" workspace-open "$slice" base
moved=feature/release-0.3-1-3
taken=feature/release-0.3-1-4
(
    cd "$root"
    jj workspace add --name "$moved" -r base "$tmp/elsewhere" >/dev/null 2>&1
    jj workspace add --name other -r base "$root/.zaf/workspaces/$moved" >/dev/null 2>&1
    jj workspace add --name another -r base "$root/.zaf/workspaces/$taken" >/dev/null 2>&1
)
expect "workspace-open refuses a slice jj holds elsewhere" 1 "jj holds workspace $moved at $tmp/elsewhere" workspace-open "$moved" base
expect "workspace-open refuses another workspace at the slice's directory" 1 "is jj workspace another, not $taken" workspace-open "$taken" base

[ "$failures" -eq 0 ] || exit 1
