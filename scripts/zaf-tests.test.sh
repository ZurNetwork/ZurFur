#!/bin/sh
# Tests for scripts/zaf-tests.sh: each branch's exact arguments, the exit status
# of the run it started (a failing test included), and the usage refusals that
# start nothing. Fake `yarn` and `cargo` on PATH record their arguments and
# working directory. Run directly, or via `just scripts-test`.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/.." && pwd)"
under_test="$script_dir/zaf-tests.sh"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT
failures=0

ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1: $2"
    failures=$((failures + 1))
}

# One stub body serves both tools: it logs "<tool> [arg] [arg] @ <cwd>" and
# exits with $STUB_STATUS.
stub="$work/stub"
cat >"$stub" <<'STUB'
#!/bin/sh
{
    printf '%s' "$(basename "$0")"
    for a in "$@"; do printf ' [%s]' "$a"; done
    printf ' @ %s\n' "$PWD"
} >>"$STUB_LOG"
exit "${STUB_STATUS:-0}"
STUB
chmod +x "$stub"
bin="$work/bin"
mkdir -p "$bin"
cp "$stub" "$bin/yarn"
cp "$stub" "$bin/cargo"

# check <name> <want status> <want log> <args...>: runs the script from /tmp
# with the fakes first on PATH; <want log> is empty when nothing must start.
check() {
    name="$1" want_status="$2" want_log="$3"
    shift 3
    : >"$work/log"
    got_status=0
    (cd /tmp && PATH="$bin:$PATH" STUB_LOG="$work/log" STUB_STATUS="${STUB_STATUS:-0}" sh "$under_test" "$@") 2>/dev/null || got_status=$?
    got_log="$(cat "$work/log")"
    if [ "$got_status" != "$want_status" ]; then
        fail "$name" "exit $got_status, wanted $want_status"
    elif [ "$got_log" != "$want_log" ]; then
        fail "$name" "ran '$got_log', wanted '$want_log'"
    else
        ok "$name"
    fi
}

spec_log="yarn [--cwd] [frontend/ui] [run] [test] [src/lib/a.spec.ts] @ $repo_root"
story_log="yarn [--cwd] [frontend/ui] [run] [test] [--project] [stories] [--project] [stories-light-still] [src/stories/A.stories.svelte] @ $repo_root"
cargo_log="cargo [test] [--workspace] [some_test] @ $repo_root"

check "a .spec.ts name runs vitest through yarn" 0 "$spec_log" src/lib/a.spec.ts
check "a .stories.svelte name runs the two story projects" 0 "$story_log" src/stories/A.stories.svelte
check "any other name runs cargo test over the workspace" 0 "$cargo_log" some_test
STUB_STATUS=3 check "a failing vitest run passes its status through" 3 "$spec_log" src/lib/a.spec.ts
STUB_STATUS=4 check "a failing story run passes its status through" 4 "$story_log" src/stories/A.stories.svelte
STUB_STATUS=101 check "a failing cargo run passes its status through" 101 "$cargo_log" some_test
check "no name exits 2 and starts nothing" 2 ""
check "two names exit 2 and start nothing" 2 "" a.spec.ts b.spec.ts
check "a name starting with - exits 2 and starts nothing" 2 "" --release

if [ "$failures" -gt 0 ]; then
    echo "$failures zaf-tests check(s) failed"
    exit 1
fi
echo "all zaf-tests checks hold"
