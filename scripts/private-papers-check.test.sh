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
expect "a Releases/ folder in capitals" 1 "Releases/x.md"
expect "a .Understand/ folder in capitals" 1 ".Understand/x.md"
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
expect "a handoff- file at the root" 1 "handoff-20261016.md"
expect "a handoff- file deeper down" 1 "docs/handoff-x.md"
expect "handoff lookalikes pass" 0 "handoff.md
my-handoff-x.md
"
expect "rulings lookalikes pass" 0 "myrulings-x.md
rulings.md
docs/rulings/x.md
"

expect "a queue.md at the root" 1 "queue.md"
expect "a queue.md deeper down" 1 ".claude/queue.md"
expect "an engineer-queue.md file" 1 "engineer-queue.md"
expect "a Queue.md in capitals" 1 "Queue.md"
expect "queue lookalikes pass" 0 "queue.mdx
myqueue.md
queue.md.bak
queue/README.md
"

expect "docs/0.1/" 1 "docs/0.1/pdd.typ"
expect "docs/v0.2/" 1 "docs/v0.2/notes.md"
expect "docs/pre-alpha-v0.2.0/" 1 "docs/pre-alpha-v0.2.0/pdd.typ"
expect "docs/alpha-1/ deeper down" 1 "docs/alpha-1/sub/deep.md"
expect "docs/V0.2/ in capitals" 1 "docs/V0.2/a.md"
expect "a suffixed docs/pre-alpha-v0.2.0-rc1/" 1 "docs/pre-alpha-v0.2.0-rc1/pdd.typ"
expect "release-folder lookalikes pass" 0 "docs/v0.2
docs/guide/x.md
docs/0.1x/a.md
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

# --unpushed outside a jj workspace has no range to check: exit 2.
(cd "$tmp" && sh "$under_test" --unpushed >/dev/null 2>&1) && got=0 || got=$?
if [ "$got" = 2 ]; then
    echo "ok   --unpushed outside a jj workspace fails closed"
else
    echo "FAIL --unpushed outside a jj workspace: expected exit 2, got $got"
    failures=$((failures + 1))
fi

# --unpushed against scratch jj repos. Each repo is isolated: JJ_CONFIG names a
# config holding only user.name/user.email, so the user's own jj settings and
# aliases never apply, and git reads no global or system config.
if command -v jj >/dev/null 2>&1; then
    printf '[user]\nname = "fixture"\nemail = "fixture@example.invalid"\n' >"$tmp/config.toml"
    export JJ_CONFIG="$tmp/config.toml" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1

    # repo <name>: a fresh jj repo at $tmp/<name>; prints its path.
    repo() {
        jj git init "$tmp/$1" >/dev/null 2>&1
        printf '%s\n' "$tmp/$1"
    }

    # unpushed <name> <repo> <expected exit> [text the output must hold]...
    unpushed() {
        name="$1"
        dir="$2"
        want="$3"
        shift 3
        out="$(cd "$dir" && sh "$under_test" --unpushed 2>&1)" && got=0 || got=$?
        if [ "$got" != "$want" ]; then
            echo "FAIL $name: expected exit $want, got $got: $out"
            failures=$((failures + 1))
            return
        fi
        for text in "$@"; do
            case "$out" in
                *"$text"*) ;;
                *)
                    echo "FAIL $name: output lacks \"$text\": $out"
                    failures=$((failures + 1))
                    return
                    ;;
            esac
        done
        echo "ok   $name"
    }

    # (a) A bookmark off @ holds a paper; @ is clean, off root().
    r="$(repo a)"
    (
        cd "$r"
        mkdir .understand && echo q >.understand/q.md
        jj commit -m "add the queue" >/dev/null 2>&1
        jj bookmark create docs/x -r @- >/dev/null 2>&1
        jj new 'root()' -m clean >/dev/null 2>&1
        echo a >a.txt
    )
    change="$(cd "$r" && jj log --no-graph -r docs/x -T 'change_id.short()')"
    unpushed "--unpushed: a bookmark away from @ holding a paper" "$r" 1 \
        "commit $change " "[docs/x] add the queue" "    .understand/q.md  (" "\`understand\` branch"

    # (b) A paper added in one commit and deleted in the next, bookmark on the
    # second, @ elsewhere: the adding commit is still in the range.
    r="$(repo b)"
    (
        cd "$r"
        mkdir retros && echo a >retros/a.md
        jj commit -m "add a retro" >/dev/null 2>&1
        rm -r retros
        jj commit -m "drop the retro" >/dev/null 2>&1
        jj bookmark create docs/y -r @- >/dev/null 2>&1
        jj new 'root()' -m clean >/dev/null 2>&1
    )
    unpushed "--unpushed: a paper added then deleted is still caught" "$r" 1 \
        "add a retro" "    retros/a.md  ("

    # (c) All clean.
    r="$(repo c)"
    (
        cd "$r"
        echo a >README.md
        jj commit -m "a readme" >/dev/null 2>&1
        jj bookmark create feature/x -r @- >/dev/null 2>&1
    )
    unpushed "--unpushed: clean commits pass" "$r" 0 "OK, no working papers added or changed in the"

    # (d) A new, never-tracked paper written into @: the snapshot picks it up.
    r="$(repo d)"
    (cd "$r" && jj describe -m wip >/dev/null 2>&1)
    echo h >"$r/handoff-20261017.md"
    unpushed "--unpushed: an untracked new paper in @ is caught" "$r" 1 \
        "    handoff-20261017.md  ("

    # (e) A paper commit already on origin is not re-flagged: a new clean
    # child that keeps the paper sends nothing new, and a child that deletes
    # it stays pushable.
    r="$(repo e)"
    git init -q --bare "$tmp/e-origin.git"
    (
        cd "$r"
        jj git remote add origin "$tmp/e-origin.git"
        mkdir retros && echo a >retros/a.md
        jj commit -m "add a retro" >/dev/null 2>&1
        jj bookmark create main -r @- >/dev/null 2>&1
        jj git push --remote origin -b main >/dev/null 2>&1
        echo b >README.md
        jj describe -m "a readme" >/dev/null 2>&1
        jj bookmark set main -r @ >/dev/null 2>&1
    )
    unpushed "--unpushed: a clean child of a paper commit on origin passes" "$r" 0 \
        "OK, no working papers added or changed in the 1 unpushed commit(s) checked"
    (
        cd "$r"
        jj new 'main@origin' -m "drop the retro" >/dev/null 2>&1
        rm -r retros
        jj bookmark set main -r @ --allow-backwards >/dev/null 2>&1
    )
    unpushed "--unpushed: deleting a paper origin already has passes" "$r" 0 \
        "OK, no working papers added or changed in the 1 unpushed commit(s) checked"

    # (f) A rename into .understand/ is caught on its target path.
    r="$(repo f)"
    (
        cd "$r"
        echo n >notes.md
        jj commit -m "some notes" >/dev/null 2>&1
        mkdir .understand && mv notes.md .understand/notes.md
        jj describe -m "file the notes" >/dev/null 2>&1
    )
    unpushed "--unpushed: a rename into .understand/ is caught" "$r" 1 \
        "file the notes" "    .understand/notes.md  ("

    # (g) HEADS narrows the range: a papers bookmark away from the named heads
    # is not this push's business.
    r="$(repo g)"
    (
        cd "$r"
        mkdir .understand && echo q >.understand/q.md
        jj commit -m "add the queue" >/dev/null 2>&1
        jj bookmark create docs/p -r @- >/dev/null 2>&1
        jj new 'root()' -m clean >/dev/null 2>&1
        echo a >a.txt
        jj bookmark create clean -r @ >/dev/null 2>&1
    )
    out="$(cd "$r" && sh "$under_test" --unpushed 'bookmarks(exact:"clean") | @' 2>&1)" && got=0 || got=$?
    if [ "$got" = 0 ]; then
        echo "ok   --unpushed HEADS: a papers bookmark outside the heads is not checked"
    else
        echo "FAIL --unpushed HEADS: expected exit 0, got $got: $out"
        failures=$((failures + 1))
    fi
    unpushed "--unpushed HEADS: the default heads still catch it" "$r" 1 "[docs/p] add the queue"
    if sh "$under_test" --unpushed "" </dev/null >/dev/null 2>&1; then
        echo "FAIL --unpushed with empty HEADS should exit non-zero"
        failures=$((failures + 1))
    else
        echo "ok   --unpushed with empty HEADS exits non-zero"
    fi
else
    echo "skip (no jj): the --unpushed fixtures"
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all private-papers-check tests passed"
