#!/bin/sh
# Tests for scripts/private-papers-check.sh. Run directly, or via `just scripts-test`.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/private-papers-check.sh"
failures=0

# expect <name> <expected exit code> <paths, one per line>
# A failing case must also name every path it was given.
expect() {
    name="$1"
    want="$2"
    out="$(printf '%s' "$3" | sh "$under_test" --stdin)" && got=0 || got=$?
    if [ "$got" != "$want" ]; then
        echo "FAIL $name: expected exit $want, got $got: $out"
        failures=$((failures + 1))
        return
    fi
    if [ "$want" = 1 ]; then
        missing=""
        for path in $3; do
            case "$out" in
                *"  $path  ("*) ;;
                *) missing="$missing $path" ;;
            esac
        done
        case "$out" in
            *"\`understand\` branch"*) ;;
            *) missing="$missing (the understand-branch line)" ;;
        esac
        if [ -n "$missing" ]; then
            echo "FAIL $name: output lacks$missing: $out"
            failures=$((failures + 1))
            return
        fi
    fi
    echo "ok   $name"
}

expect "no paths passes" 0 ""
expect "today's docs/ passes" 0 "docs/design-index.md
docs/NODE.json
"

expect "a .understand/ folder at the root" 1 ".understand/handoff.md"
expect "a .understand/ folder deeper down" 1 "backend/.understand/queue-notes.md"
expect "a .teams/ folder at the root" 1 ".teams/roster.md"
expect "a .teams/ folder deeper down" 1 "docs/.teams/a.md"
expect "lookalike hidden folders pass" 0 ".understandx/a.md
.teamsy/a.md
x.understand/a.md
.teams.md
"

expect "a releases/ folder" 1 "releases/0.1.md"
expect "a releases/ folder deeper down" 1 "docs/releases/notes.md"
expect "a retros/ folder" 1 "retros/2026-10.md"
expect "a retros/ folder deeper down" 1 ".claude/retros/x.md"
expect "release and retro lookalikes pass" 0 "frontend/web/src/release.ts
frontend/web/src/releases
docs/retros.md
backend/releasesx/a.rs
"

expect "a rulings- file at the root" 1 "rulings-2026-10.md"
expect "a rulings- file deeper down" 1 "docs/rulings-q2.md"
expect "rulings lookalikes pass" 0 "myrulings-x.md
rulings.md
docs/rulings/x.md
"

expect "a queue.md at the root" 1 "queue.md"
expect "a queue.md deeper down" 1 ".claude/queue.md"
expect "queue lookalikes pass" 0 "queue.mdx
myqueue.md
queue.md.bak
queue/README.md
"

expect "docs/0.1/" 1 "docs/0.1/pdd.typ"
expect "docs/v0.2/" 1 "docs/v0.2/notes.md"
expect "docs/pre-alpha-v0.2.0/" 1 "docs/pre-alpha-v0.2.0/pdd.typ"
expect "docs/alpha-1/ deeper down" 1 "docs/alpha-1/sub/deep.md"
expect "release-folder lookalikes pass" 0 "docs/v0.2
docs/guide/x.md
docs/0.1x/a.md
docs/V0.2/a.md
docs/-v0.2/a.md
docs/0./a.md
docs/pre_alpha-v0.2/a.md
backend/docs/0.1/a.md
"

expect "a quoted name git could not print plainly" 1 "\"docs/0.1/\\303\\251.md\""

out="$(printf 'README.md\nretros/a.md\ndocs/design-index.md\n' | sh "$under_test" --stdin)" && got=0 || got=$?
if [ "$got" = 1 ] && printf '%s\n' "$out" | grep -qF '  retros/a.md  (' &&
    ! printf '%s\n' "$out" | grep -qE 'README\.md|design-index'; then
    echo "ok   one offender among clean paths is the only one listed"
else
    echo "FAIL one offender among clean paths: exit $got: $out"
    failures=$((failures + 1))
fi

if sh "$under_test" --bogus </dev/null >/dev/null 2>&1; then
    echo "FAIL an unknown flag should exit non-zero"
    failures=$((failures + 1))
else
    echo "ok   an unknown flag exits non-zero"
fi

# Outside any jj workspace or git checkout there is nothing to list: exit 2.
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
(cd "$tmp" && GIT_CEILING_DIRECTORIES="$tmp" sh "$under_test" >/dev/null 2>&1) && got=0 || got=$?
if [ "$got" = 2 ]; then
    echo "ok   no listing fails closed"
else
    echo "FAIL no listing fails closed: expected exit 2, got $got"
    failures=$((failures + 1))
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all private-papers-check tests passed"
