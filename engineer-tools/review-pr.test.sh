#!/bin/sh
# Tests for engineer-tools/review-pr.sh against a scratch colocated jj checkout
# of a scratch origin, with `gh` stubbed. Run directly, or via
# `just scripts-test`. Needs git and jj; skips without jj (CI has none).
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/review-pr.sh"
if ! command -v jj >/dev/null 2>&1; then
    echo "skip review-pr tests (jj not installed)"
    exit 0
fi

work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT
failures=0

# An isolated jj and git identity, so the user's own config can't interfere.
printf '[user]\nname = "t"\nemail = "t@example.com"\n' >"$work/jj.toml"
export JJ_CONFIG="$work/jj.toml"
export GIT_CONFIG_GLOBAL="$work/gitconfig"
git config --file "$GIT_CONFIG_GLOBAL" user.name t
git config --file "$GIT_CONFIG_GLOBAL" user.email t@example.com
git config --file "$GIT_CONFIG_GLOBAL" init.defaultBranch main
export XDG_STATE_HOME="$work/state"

# gh stub: `gh pr view <N> …` prints $STUB_DIR/pr-<N>.txt ("<base> <head>").
mkdir -p "$work/bin" "$work/stub"
cat >"$work/bin/gh" <<'STUB'
#!/bin/sh
[ "$1 $2" = "pr view" ] && [ -f "$STUB_DIR/pr-$3.txt" ] || exit 1
cat "$STUB_DIR/pr-$3.txt"
STUB
chmod +x "$work/bin/gh"
export STUB_DIR="$work/stub"
export PATH="$work/bin:$PATH"

# The origin: main with app.rs, and two PR branches.
git init -q --bare "$work/origin.git"
seed="$work/seed"
git clone -q "$work/origin.git" "$seed" 2>/dev/null
printf 'fn main() {}\n' >"$seed/app.rs"
git -C "$seed" add -A
git -C "$seed" commit -qm base
git -C "$seed" push -q origin HEAD:main
git -C "$seed" checkout -qb pr1
printf 'fn main() { run(); }\n' >"$seed/app.rs"
git -C "$seed" commit -qam "pr one"
git -C "$seed" push -q origin pr1
git -C "$seed" checkout -q main
git -C "$seed" checkout -qb pr2
printf 'two\n' >"$seed/two.md"
git -C "$seed" add -A
git -C "$seed" commit -qm "pr two"
git -C "$seed" push -q origin pr2
printf 'main %s\n' "$(git -C "$seed" rev-parse pr1)" >"$STUB_DIR/pr-1.txt"
printf 'main %s\n' "$(git -C "$seed" rev-parse pr2)" >"$STUB_DIR/pr-2.txt"

# The Engineer's colocated checkout, with a draft change open.
co="$work/zurfur"
jj git clone --colocate "file://$work/origin.git" "$co" >/dev/null 2>&1
export REVIEW_PR_ROOT="$co"
(cd "$co" && jj new main@origin -m "engineer draft" >/dev/null 2>&1 && printf 'draft\n' >draft.txt && jj status >/dev/null)
draft="$(cd "$co" && jj log --no-graph -r @ -T change_id)"

# run <dir> <args…> — runs the script there; leaves $got and $work/out.
run() {
    dir="$1"
    shift
    set +e
    (cd "$dir" && bash "$under_test" "$@") >"$work/out" 2>&1
    got=$?
    set -e
}

# check <name> <expected exit> <expected output fragment> <args…> — in $co.
check() {
    name="$1"
    want="$2"
    needle="$3"
    shift 3
    run "$co" "$@"
    if [ "$got" -ne "$want" ] || ! grep -qF -- "$needle" "$work/out"; then
        echo "FAIL $name: exit $got (want $want); output:"
        awk '{ print "  " $0 }' "$work/out"
        failures=$((failures + 1))
    else
        echo "ok   $name"
    fi
}

# assert <name> <test command…>
assert() {
    name="$1"
    shift
    if "$@"; then
        echo "ok   $name"
    else
        echo "FAIL $name"
        failures=$((failures + 1))
    fi
}

at() { (cd "$co" && jj log --no-graph -r @ -T "$1"); }
on_change() { [ "$(at change_id)" = "$1" ]; }
file_has() { grep -qF -- "$2" "$co/$1"; }

# Flags.
check "--start needs --pr" 2 "--start needs --pr" --start
check "two actions are refused" 2 "can't be used together" --start --stop --pr=1
check "--yes only with --stop" 2 "--yes only goes with --stop" --pause --yes
check "a non-numeric --pr is refused" 2 "wants a PR number" --start --pr=abc

# Where it runs.
mkdir -p "$work/elsewhere"
(cd "$work/elsewhere" && jj git init >/dev/null 2>&1)
run "$work/elsewhere" --start --pr=1
assert "another checkout is refused" sh -c "[ $got -eq 1 ] && grep -qF 'run this in' '$work/out'"
(cd "$co" && jj workspace add "$work/ws" >/dev/null 2>&1)
REVIEW_PR_ROOT="$work/ws" run "$work/ws" --start --pr=1
assert "a workspace without .git is refused" sh -c "[ $got -eq 1 ] && grep -qF 'has no .git' '$work/out'"

# Start.
check "with no action and no reviews, it says so" 0 "No reviews"
check "start opens the review" 0 "Next: code $co" --start --pr=1
review1="$(at change_id)"
assert "the review change holds the PR head's files" file_has app.rs "run();"
assert "the review change is not the draft" sh -c "[ '$review1' != '$draft' ]"
assert "the review change has no bookmark" sh -c "[ -z \"\$(cd '$co' && jj log --no-graph -r @ -T bookmarks)\" ]"
assert "the review shows the PR's diff" sh -c "cd '$co' && jj diff --name-only | grep -qx app.rs"
check "a second review while one is active is refused" 1 "review #1 is active" --start --pr=2

# Pause and resume with a note.
printf '// REVIEW: why run()?\n' >>"$co/app.rs"
check "pause goes back" 0 "Paused the review of #1" --pause
assert "pause is back on the draft" on_change "$draft"
assert "the draft's file is back" file_has draft.txt draft
check "the list shows the paused review" 0 "#1 paused"
check "stop with no active review needs --pr" 1 "no active review" --stop
check "resume" 0 "Resumed the review of #1" --start --pr=1
assert "resume is on the review change" on_change "$review1"
assert "the note survived the pause" file_has app.rs "// REVIEW: why run()?"

# The PR head moves while paused.
check "pause again" 0 "Paused" --pause
git -C "$seed" checkout -q pr1
printf 'fn main() { run(); stop(); }\n' >"$seed/app.rs"
git -C "$seed" commit -qam "pr one, more"
git -C "$seed" push -q origin pr1
printf 'main %s\n' "$(git -C "$seed" rev-parse pr1)" >"$STUB_DIR/pr-1.txt"
check "a moved head warns with old..new" 0 "head moved since this review started" --start --pr=1
assert "the note survived the moved head" file_has app.rs "// REVIEW: why run()?"

# Stop.
check "stop with notes is refused" 1 "--stop --yes drops them" --stop
assert "a refused stop stays on the review" on_change "$review1"
check "stop --yes drops the review" 0 "Stopped the review of #1" --stop --yes
assert "stop is back on the draft" on_change "$draft"
assert "the review change is abandoned" sh -c "[ -z \"\$(cd '$co' && jj log --no-graph -r '$review1' -T change_id 2>/dev/null)\" ]"
assert "the state is gone" sh -c "[ ! -e '$XDG_STATE_HOME/zurfur/reviews/1.state' ]"

check "start #2" 0 "Review of #2" --start --pr=2
check "stop without notes needs no --yes" 0 "Stopped the review of #2" --stop
assert "back on the draft after #2" on_change "$draft"

# An empty change before --start: jj drops it, so stop returns to a new
# change on the same parent.
(cd "$co" && jj new main@origin >/dev/null 2>&1)
check "start from an empty change" 0 "Review of #2" --start --pr=2
check "stop returns to a new change on the same parent" 0 "Back on a new change" --stop
assert "the new change sits on main" sh -c "[ \"\$(cd '$co' && jj log --no-graph -r @- -T commit_id)\" = \"\$(cd '$co' && jj log --no-graph -r main@origin -T commit_id)\" ]"

# A conflicted working copy is refused.
(cd "$co" && jj new main@origin -m a >/dev/null 2>&1 && printf 'a\n' >c.txt && jj new main@origin -m b >/dev/null 2>&1 && printf 'b\n' >c.txt && jj status >/dev/null)
(cd "$co" && jj new 'description(exact:"a\n")' 'description(exact:"b\n")' >/dev/null 2>&1)
check "a conflicted working copy is refused" 1 "has conflicts" --start --pr=1

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all review-pr tests passed"
