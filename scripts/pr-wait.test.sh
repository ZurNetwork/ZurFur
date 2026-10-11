#!/bin/sh
# Tests for scripts/pr-wait.sh against a stubbed `gh`. Run directly, or via
# `just scripts-test`. Needs bash and jq.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/pr-wait.sh"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

# The stub answers from files in $STUB_DIR: checks.json, reviews.json,
# comments.json, and repo.txt for `gh repo view` (absent: it fails, as outside
# a git checkout). A missing checks.json mimics "no checks yet". The jj stub
# prints jj-remotes.txt for `jj git remote list` (absent: it fails).
mkdir -p "$work/bin"
cat >"$work/bin/jj" <<'STUB'
#!/bin/sh
[ "$1 $2 $3" = "git remote list" ] && [ -f "$STUB_DIR/jj-remotes.txt" ] || exit 1
cat "$STUB_DIR/jj-remotes.txt"
STUB
chmod +x "$work/bin/jj"
cat >"$work/bin/gh" <<'STUB'
#!/bin/sh
dir="$STUB_DIR"
case "$1 $2" in
    "repo view")
        [ -f "$dir/repo.txt" ] || exit 1
        cat "$dir/repo.txt"
        ;;
    "pr view")
        # base.txt, if present, answers the base query (default: main).
        case "$*" in
            *baseRefName*)
                if [ -f "$dir/base.txt" ]; then cat "$dir/base.txt"; else echo main; fi
                exit 0
                ;;
        esac
        # heads.txt, if present: one head per call, the last one repeating.
        if [ -f "$dir/heads.txt" ]; then
            n=$(cat "$dir/head-calls" 2>/dev/null || echo 0)
            n=$((n + 1))
            echo "$n" >"$dir/head-calls"
            total=$(wc -l <"$dir/heads.txt")
            [ "$n" -gt "$total" ] && n=$total
            awk -v n="$n" 'NR == n' "$dir/heads.txt"
        else
            echo "abcdef1234567890"
        fi
        ;;
    "pr checks")
        [ -f "$dir/checks.err" ] && { cat "$dir/checks.err" >&2; exit 1; }
        [ -f "$dir/checks.json" ] || { echo "no checks reported on the 'x' branch" >&2; exit 1; }
        cat "$dir/checks.json"
        ;;
    "api "*)
        jq_filter=""
        prev=""
        for arg in "$@"; do
            [ "$prev" = "--jq" ] && jq_filter="$arg"
            prev="$arg"
        done
        case "$2" in
            */reviews/*/comments) file="$dir/comments.json" ;;
            */reviews) file="$dir/reviews.json" ;;
            *) exit 1 ;;
        esac
        jq -r "$jq_filter" "$file"
        ;;
    *) exit 1 ;;
esac
STUB
chmod +x "$work/bin/gh"

failures=0

# run_case <name> <expected exit> <expected output fragment> [env assignments…]
# PR_WAIT_REPO is o/r unless an assignment overrides it (PR_WAIT_REPO= unsets).
run_case() {
    name="$1"
    want="$2"
    needle="$3"
    shift 3
    set +e
    env PATH="$work/bin:$PATH" STUB_DIR="$work/case" PR_WAIT_REPO=o/r PR_WAIT_INTERVAL=0 PR_WAIT_TIMEOUT=0 PR_WAIT_MAX_ERRORS=1 "$@" \
        bash "$under_test" 7 >"$work/out" 2>&1
    got=$?
    set -e
    if [ "$got" -ne "$want" ] || ! grep -qF "$needle" "$work/out"; then
        echo "FAIL $name: exit $got (want $want); output:"
        awk '{ print "  " $0 }' "$work/out"
        failures=$((failures + 1))
    else
        echo "ok   $name"
    fi
}

new_case() {
    rm -rf "$work/case"
    mkdir -p "$work/case"
    echo '[]' >"$work/case/reviews.json"
    echo '[]' >"$work/case/comments.json"
}

head=abcdef1234567890
copilot_review() { # copilot_review <id> <commit>…
    printf '[%s]\n' "$(printf '{"id":%s,"commit_id":"%s","user":{"login":"copilot-pull-request-reviewer[bot]"}}' "$1" "$2")"
}

new_case
echo '[{"name":"test","bucket":"pass"},{"name":"web","bucket":"skipping"}]' >"$work/case/checks.json"
copilot_review 5 "$head" >"$work/case/reviews.json"
echo '[{"id":1},{"id":2}]' >"$work/case/comments.json"
run_case "green and reviewed" 0 "Copilot: reviewed abcdef1, 2 inline comment(s)"
run_case "green counts passed and skipped" 0 "CI: green, 1 passed, 1 skipped"

new_case
echo '[{"name":"test","bucket":"fail"},{"name":"fmt","bucket":"pass"}]' >"$work/case/checks.json"
run_case "red names the failed job" 1 "CI: red, failed: test"
run_case "no Copilot review at all" 1 "Copilot: no review of abcdef1 by the timeout (none yet)"

new_case
echo '[{"name":"test","bucket":"pending"}]' >"$work/case/checks.json"
run_case "pending at the timeout" 2 "CI: still running at the timeout: test"

new_case
echo '[{"name":"test","bucket":"pass"}]' >"$work/case/checks.json"
copilot_review 4 1111111222 >"$work/case/reviews.json"
run_case "a Copilot review of an older head isn't done" 2 "Copilot: no review of abcdef1 by the timeout (last reviewed 1111111)"
run_case "PR_WAIT_COPILOT=0 doesn't wait for Copilot" 0 "Copilot: not waited for (PR_WAIT_COPILOT=0)" PR_WAIT_COPILOT=0
echo "feature/my-den" >"$work/case/base.txt"
run_case "a slice PR (base isn't main) doesn't wait for Copilot" 0 "Copilot: not waited for (a PR into feature/my-den)"
run_case "PR_WAIT_COPILOT=0 still skips on a slice" 0 "Copilot: not waited for (PR_WAIT_COPILOT=0)" PR_WAIT_COPILOT=0

new_case
echo '[{"name":"test","bucket":"pass"}]' >"$work/case/checks.json"
quota="Copilot was unable to review this pull request because the user who requested the review has reached their quota limit."
printf '[{"id":8,"commit_id":"%s","body":"%s","user":{"login":"copilot-pull-request-reviewer[bot]"}}]\n' "$head" "$quota" >"$work/case/reviews.json"
run_case "a quota notice on the head is not a review (exit 4)" 4 "Copilot: unavailable (the user who requested the review has reached their quota limit)"
printf '[{"id":8,"commit_id":"%s","body":"%s","user":{"login":"Copilot"}},{"id":9,"commit_id":"%s","body":"Looks fine.","user":{"login":"Copilot"}}]\n' "$head" "$quota" "$head" >"$work/case/reviews.json"
run_case "a real review after a quota notice counts" 0 "Copilot: reviewed abcdef1"
printf '[{"id":8,"commit_id":"%s","body":"Copilot wasn%st able to review any files in this pull request. --- Add a code-review agent skill.","user":{"login":"Copilot"}}]\n' "$head" "'" >"$work/case/reviews.json"
run_case "the no-files notice is not a review (exit 4)" 4 "Copilot: unavailable (Copilot wasn't able to review any files in this pull request)"
echo '[{"name":"test","bucket":"fail"}]' >"$work/case/checks.json"
printf '[{"id":8,"commit_id":"%s","body":"%s","user":{"login":"Copilot"}}]\n' "$head" "$quota" >"$work/case/reviews.json"
run_case "red CI with a quota notice is still red" 1 "Copilot: unavailable"

new_case
run_case "no checks yet counts as pending" 2 "CI: still running at the timeout"

new_case
echo "HTTP 502: Bad Gateway" >"$work/case/checks.err"
run_case "a GitHub error on the checks exits 3" 3 "cannot read the checks"

new_case
echo '[{"name":"test","bucket":"pass"}]' >"$work/case/checks.json"
echo 'not json' >"$work/case/reviews.json"
run_case "a GitHub error on the reviews exits 3" 3 "cannot read the Copilot reviews"

new_case
echo '[{"name":"test","bucket":"pass"}]' >"$work/case/checks.json"
printf 'aaaaaaa000\nbbbbbbb111\n' >"$work/case/heads.txt"
copilot_review 6 bbbbbbb111 >"$work/case/reviews.json"
run_case "a push while waiting waits again for the new head" 0 "this verdict is for bbbbbbb"

new_case
echo 'not json' >"$work/case/checks.json"
run_case "an unparsable checks answer is a GitHub error" 3 "cannot read the checks"

# Which repository: PR_WAIT_REPO, else gh (a git checkout), else jj's remote.
new_case
echo '[{"name":"test","bucket":"pass"}]' >"$work/case/checks.json"
copilot_review 5 "$head" >"$work/case/reviews.json"
echo "o2/r2" >"$work/case/repo.txt"
run_case "the repo comes from gh in a git checkout" 0 "PR #7 in o2/r2" PR_WAIT_REPO=
rm "$work/case/repo.txt"
printf 'origin git@github.com:Zur/Fur.git\nupstream https://github.com/x/y\n' >"$work/case/jj-remotes.txt"
run_case "the repo comes from jj's origin in a jj workspace" 0 "PR #7 in Zur/Fur" PR_WAIT_REPO=
printf 'fork https://github.com/Zur/Fork.git\n' >"$work/case/jj-remotes.txt"
run_case "jj's only remote is used when there is no origin" 0 "PR #7 in Zur/Fork" PR_WAIT_REPO=
printf 'origin https://gitlab.com/Zur/Fur.git\n' >"$work/case/jj-remotes.txt"
run_case "a remote that isn't GitHub is a usage error" 3 "set PR_WAIT_REPO" PR_WAIT_REPO=
rm "$work/case/jj-remotes.txt"
run_case "no gh repo and no jj remote is a usage error" 3 "set PR_WAIT_REPO" PR_WAIT_REPO=

set +e
bash "$under_test" notanumber >/dev/null 2>&1
got=$?
set -e
if [ "$got" -eq 3 ]; then
    echo "ok   usage error exits 3"
else
    echo "FAIL usage error: exit $got (want 3)"
    failures=$((failures + 1))
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all pr-wait tests passed"
