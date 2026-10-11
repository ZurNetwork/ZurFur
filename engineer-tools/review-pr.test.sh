#!/bin/sh
# Tests for engineer-tools/review-pr.sh against a scratch colocated jj checkout
# of a scratch origin, with `gh` stubbed. Run directly, or via
# `just engineer-tools-test`. Needs git and jj; skips without jj (CI has none).
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

# gh stub, answering from $STUB_DIR:
#   gh pr view <N> …                    pr-<N>.txt ("<base> <head> <url>")
#   gh api …/pulls/<N>/files --jq F     F over files-<N>.json
#   gh api …/pulls/<N>/reviews --jq F   F over reviews-<N>.json (default [])
#   gh api -X POST …/pulls/<N>/reviews  saves --input to posted-<N>.json
mkdir -p "$work/bin" "$work/stub"
cat >"$work/bin/gh" <<'STUB'
#!/bin/sh
d="$STUB_DIR"
if [ "$1 $2" = "pr view" ]; then
    [ -f "$d/pr-$3.txt" ] || exit 1
    cat "$d/pr-$3.txt"
    exit 0
fi
[ "$1" = api ] || exit 1
shift
method=GET
input=""
filter=""
path=""
while [ $# -gt 0 ]; do
    case "$1" in
        -X | --method) method="$2"; shift ;;
        --input) input="$2"; shift ;;
        --jq) filter="$2"; shift ;;
        repos/*) path="$1" ;;
    esac
    shift
done
n="$(printf '%s' "$path" | awk -F/ '{ print $5 }')"
case "$method $path" in
    "POST "*/reviews) cp "$input" "$d/posted-$n.json" && echo '{"id":1}' ;;
    "GET "*/reviews) [ -f "$d/reviews-$n.json" ] || echo '[]' >"$d/reviews-$n.json"; jq -r "$filter" "$d/reviews-$n.json" ;;
    "GET "*/files) jq -r "$filter" "$d/files-$n.json" ;;
    *) exit 1 ;;
esac
STUB
chmod +x "$work/bin/gh"
export STUB_DIR="$work/stub"
export PATH="$work/bin:$PATH"

# The origin: main with app.rs, and two PR branches.
git init -q --bare "$work/origin.git"
seed="$work/seed"
git clone -q "$work/origin.git" "$seed" 2>/dev/null
printf 'fn main() {}\n' >"$seed/app.rs"
printf 'l%s\n' 1 2 3 4 5 6 7 8 9 10 >"$seed/lib.rs"
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
git -C "$seed" checkout -q main
git -C "$seed" checkout -qb pr3
printf 'l1\nl2\nl3\nl4\nL5 changed\nl6\nl7\nl8\nl9\nl10\n' >"$seed/lib.rs"
git -C "$seed" commit -qam "pr three"
git -C "$seed" push -q origin pr3
pr3_head="$(git -C "$seed" rev-parse pr3)"
printf 'main %s https://github.com/o/r/pull/3\n' "$pr3_head" >"$STUB_DIR/pr-3.txt"
# GitHub's diff of #3: one hunk, lib.rs lines 2-8 on the right.
printf '[{"filename":"lib.rs","patch":"@@ -2,7 +2,7 @@ l1\\n l2"}]\n' >"$STUB_DIR/files-3.json"
printf 'main %s\n' "$(git -C "$seed" rev-parse pr1)" >"$STUB_DIR/pr-1.txt"
printf 'main %s\n' "$(git -C "$seed" rev-parse pr2)" >"$STUB_DIR/pr-2.txt"

# The Engineer's colocated checkout, with a draft change open.
co="$work/zurfur"
jj git clone --colocate "file://$work/origin.git" "$co" >/dev/null 2>&1
(cd "$co" && jj new main@origin -m "engineer draft" >/dev/null 2>&1 && printf 'draft\n' >draft.txt && jj status >/dev/null)
draft="$(cd "$co" && jj log --no-graph -r @ -T change_id)"

run_env=""

# run <dir> <args…> — runs the script there; leaves $got and $work/out.
run() {
    dir="$1"
    shift
    set +e
    (cd "$dir" && env $run_env bash "$under_test" "$@") >"$work/out" 2>&1
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
check "--publish with another action is refused" 2 "can't be used together" --publish --stop

# Where it runs: any colocated jj checkout; REVIEW_PR_ROOT only narrows it.
mkdir -p "$work/plain"
run "$work/plain" --start --pr=1
assert "outside jj is refused" sh -c "[ $got -eq 1 ] && grep -qF 'not in a jj checkout' '$work/out'"
(cd "$co" && jj workspace add "$work/ws" >/dev/null 2>&1)
run "$work/ws" --start --pr=1
assert "a workspace without .git is refused" sh -c "[ $got -eq 1 ] && grep -qF 'has no .git directory' '$work/out'"
jj git clone --colocate "file://$work/origin.git" "$work/other" >/dev/null 2>&1
run_env="REVIEW_PR_ROOT=$co"
run "$work/other" --start --pr=2
assert "REVIEW_PR_ROOT refuses another checkout" sh -c "[ $got -eq 1 ] && grep -qF 'REVIEW_PR_ROOT says' '$work/out'"
run_env=""
run "$work/other" --start --pr=2
assert "another colocated checkout works without the override" sh -c "[ $got -eq 0 ] && grep -qF 'Review of #2' '$work/out'"
run "$work/other" --stop --pr=2
assert "and stops there" sh -c "[ $got -eq 0 ] && grep -qF 'Stopped the review of #2' '$work/out'"

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

# A paused review reopened by hand: stop leaves it before abandoning it.
check "start #2 again" 0 "Review of #2" --start --pr=2
review2="$(at change_id)"
check "pause #2" 0 "Paused" --pause
(cd "$co" && jj edit "$review2" >/dev/null 2>&1)
check "stop a paused review from inside it" 0 "Stopped the review of #2" --stop --pr=2
assert "that stop is back on the draft" on_change "$draft"

# A failed restore leaves state behind, so --stop gets back.
real_jj="$(command -v jj)"
mkdir -p "$work/failbin"
printf '#!/bin/sh\n[ "$1" = restore ] && exit 1\nexec "%s" "$@"\n' "$real_jj" >"$work/failbin/jj"
chmod +x "$work/failbin/jj"
run_env="PATH=$work/failbin:$PATH"
check "a failed restore names the way back" 1 "--stop --pr=2 --yes goes back to $draft" --start --pr=2
run_env=""
check "stop after a failed restore" 0 "Stopped the review of #2" --stop --pr=2 --yes
assert "that stop is back on the draft too" on_change "$draft"

# An empty change before --start: jj drops it, so stop returns to a new
# change on the same parent.
(cd "$co" && jj new main@origin >/dev/null 2>&1)
check "start from an empty change" 0 "Review of #2" --start --pr=2
check "stop returns to a new change on the same parent" 0 "Back on a new change" --stop
assert "the new change sits on main" sh -c "[ \"\$(cd '$co' && jj log --no-graph -r @- -T commit_id)\" = \"\$(cd '$co' && jj log --no-graph -r main@origin -T commit_id)\" ]"

# Publish: notes become one pending review.
check "start #3" 0 "Review of #3" --start --pr=3
review3="$(at change_id)"
printf -- '-- REVIEW: top\nl1\nl2\nl3\nl4\nL5 changed\n// REVIEW: why change l5?\n  // REVIEW: second line\nl6\nl7\n<!-- REVIEW: html note -->\nl8\nl9 edited\nl10\n# REVIEW: outside the hunk\n' >"$co/lib.rs"
printf '# REVIEW: new file note\n' >"$co/new.md"
check "publish posts one pending review" 0 "2 line comment(s), 3 in the body" --publish
posted="$STUB_DIR/posted-3.json"
assert "the review is pending (no event) on the PR head" sh -c "jq -e --arg h '$pr3_head' '.commit_id == \$h and (has(\"event\") | not)' '$posted' >/dev/null"
assert "consecutive notes on line 5 join into one comment" sh -c "jq -e '.comments[0] == {path: \"lib.rs\", line: 5, side: \"RIGHT\", body: \"why change l5?\\nsecond line\"}' '$posted' >/dev/null"
assert "an HTML-comment note maps to line 7" sh -c "jq -e '.comments[1] == {path: \"lib.rs\", line: 7, side: \"RIGHT\", body: \"html note\"}' '$posted' >/dev/null"
assert "only the notes inside the hunk are line comments" sh -c "jq -e '.comments | length == 2' '$posted' >/dev/null"
assert "a note outside the hunk goes into the body" sh -c "jq -r .body '$posted' | grep -qF -- '- \`lib.rs:10\` — outside the hunk'"
assert "a note above line 1 goes into the body at line 1" sh -c "jq -r .body '$posted' | grep -qF -- '- \`lib.rs:1\` — top'"
assert "a note in a file the head lacks goes into the body" sh -c "jq -r .body '$posted' | grep -qF -- '- \`new.md (new file)\` — new file note'"
assert "a code edit is not a note" sh -c "! grep -qF 'l9 edited' '$posted'"
assert "publishing leaves the review as it was" sh -c "[ \"\$(cd '$co' && jj log --no-graph -r @ -T change_id)\" = '$review3' ] && grep -qF 'REVIEW: why change l5?' '$co/lib.rs'"
rm -f "$posted"

echo '[{"id":7,"state":"PENDING"}]' >"$STUB_DIR/reviews-3.json"
check "an existing pending review is refused" 1 "already have a pending review" --publish
rm -f "$STUB_DIR/reviews-3.json"
printf 'main 0123456789abcdef0123456789abcdef01234567 https://github.com/o/r/pull/3\n' >"$STUB_DIR/pr-3.txt"
check "a moved head is refused" 1 "--stop it and --start again" --publish --pr=3
printf 'main %s https://github.com/o/r/pull/3\n' "$pr3_head" >"$STUB_DIR/pr-3.txt"
assert "nothing was posted by the refusals" sh -c "[ ! -e '$posted' ]"
check "stop #3" 0 "Stopped the review of #3" --stop --yes
check "start #2 for the no-notes case" 0 "Review of #2" --start --pr=2
check "a review with no notes is refused" 1 "no REVIEW: notes" --publish
check "stop #2" 0 "Stopped the review of #2" --stop

# A conflicted working copy is refused.
(cd "$co" && jj new main@origin -m a >/dev/null 2>&1 && printf 'a\n' >c.txt && jj new main@origin -m b >/dev/null 2>&1 && printf 'b\n' >c.txt && jj status >/dev/null)
(cd "$co" && jj new 'description(exact:"a\n")' 'description(exact:"b\n")' >/dev/null 2>&1)
check "a conflicted working copy is refused" 1 "has conflicts" --start --pr=1

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all review-pr tests passed"
