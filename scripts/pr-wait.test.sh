#!/bin/sh
# Tests for scripts/pr-wait.sh against a stubbed `gh`. Run directly, or via
# `just scripts-test`. Needs bash and jq.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/pr-wait.sh"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

# The stub answers from files in $STUB_DIR: checks.json, requested.json,
# reviews.json, comments.json. A missing checks.json mimics "no checks yet".
mkdir -p "$work/bin"
cat >"$work/bin/gh" <<'STUB'
#!/bin/sh
dir="$STUB_DIR"
case "$1 $2" in
    "pr view") echo "abcdef1234567890" ;;
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
            */requested_reviewers) file="$dir/requested.json" ;;
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

# run_case <name> <expected exit> <expected output fragment>
run_case() {
    name="$1"
    want="$2"
    needle="$3"
    set +e
    PATH="$work/bin:$PATH" STUB_DIR="$work/case" PR_WAIT_REPO=o/r PR_WAIT_INTERVAL=0 PR_WAIT_TIMEOUT=0 PR_WAIT_MAX_ERRORS=1 \
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
    echo '{"users":[],"teams":[]}' >"$work/case/requested.json"
    echo '[]' >"$work/case/reviews.json"
    echo '[]' >"$work/case/comments.json"
}

new_case
echo '[{"name":"test","bucket":"pass"},{"name":"web","bucket":"skipping"}]' >"$work/case/checks.json"
echo '[{"id":5,"commit_id":"abcdef1234","user":{"login":"copilot-pull-request-reviewer[bot]"}}]' >"$work/case/reviews.json"
echo '[{"id":1},{"id":2}]' >"$work/case/comments.json"
run_case "green and reviewed" 0 "Copilot: reviewed abcdef1, 2 inline comment(s)"
run_case "green counts passed and skipped" 0 "CI: green, 1 passed, 1 skipped"

new_case
echo '[{"name":"test","bucket":"fail"},{"name":"fmt","bucket":"pass"}]' >"$work/case/checks.json"
run_case "red names the failed job" 1 "CI: red, failed: test"
run_case "never requested" 1 "Copilot: not requested, no review"

new_case
echo '[{"name":"test","bucket":"pending"}]' >"$work/case/checks.json"
echo '{"users":[{"login":"Copilot"}],"teams":[]}' >"$work/case/requested.json"
run_case "pending at the timeout" 2 "CI: still running at the timeout: test"
run_case "Copilot still requested" 2 "Copilot: still requested at the timeout"

new_case
run_case "no checks yet counts as pending" 2 "CI: still running at the timeout"

new_case
echo "HTTP 502: Bad Gateway" >"$work/case/checks.err"
run_case "a GitHub error on the checks exits 3" 3 "cannot read the checks"

new_case
echo '[{"name":"test","bucket":"pass"}]' >"$work/case/checks.json"
echo 'not json' >"$work/case/requested.json"
run_case "a GitHub error on the reviewers exits 3" 3 "cannot read the requested reviewers"

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
