#!/bin/sh
# Tests for scripts/jj-push.sh: its argument guard and its private-papers
# check (including papers only a tag reaches). Run directly, or via `just scripts-test`. Needs jj; without it (CI's
# `hooks` job) it prints a skip line and passes.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
failures=0

if ! command -v jj >/dev/null 2>&1; then
    echo "skip (no jj): the jj-push fixtures"
    exit 0
fi

# One scratch repo, isolated: JJ_CONFIG names a config holding only
# user.name/user.email, so the user's own jj settings and `push` alias never
# apply, and git reads no global or system config. origin is a bare repo.
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
printf '[user]\nname = "fixture"\nemail = "fixture@example.invalid"\n' >"$tmp/config.toml"
export JJ_CONFIG="$tmp/config.toml" GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
repo="$tmp/repo"
origin="$tmp/origin.git"
git init -q --bare "$origin"
jj git init "$repo" >/dev/null 2>&1

# docs/p: a commit off root() that adds a paper. @ (bookmark `clean`): a clean
# commit off root() that carries the two scripts under test (no .rs/.toml, so
# the cargo gate skips itself).
(
    cd "$repo"
    jj git remote add origin "$origin"
    mkdir .understand && echo q >.understand/q.md
    jj commit -m "add a paper" >/dev/null 2>&1
    jj bookmark create docs/p -r @- >/dev/null 2>&1
    jj new 'root()' -m "clean work" >/dev/null 2>&1
    mkdir scripts
    cp "$script_dir/jj-push.sh" "$script_dir/private-papers-check.sh" scripts/
    jj bookmark create clean -r @ >/dev/null 2>&1
)

# pushed: how many refs origin holds (0 means nothing was ever pushed).
pushed() {
    git --git-dir="$origin" for-each-ref | wc -l | tr -d ' '
}

# run <name> <expected exit> <text the output must hold> -- <args>...
# Also fails when the output says the push ran when it was not meant to.
run() {
    name="$1"
    want="$2"
    text="$3"
    shift 4
    out="$(cd "$repo" && sh scripts/jj-push.sh "$@" 2>&1)" && got=0 || got=$?
    if [ "$got" != "$want" ]; then
        echo "FAIL $name: expected exit $want, got $got: $out"
        failures=$((failures + 1))
        return
    fi
    case "$out" in
        *"$text"*) ;;
        *)
            echo "FAIL $name: output lacks \"$text\": $out"
            failures=$((failures + 1))
            return
            ;;
    esac
    if [ "$want" != 0 ]; then
        case "$out" in
            *"gate green"*)
                echo "FAIL $name: reached the push: $out"
                failures=$((failures + 1))
                return
                ;;
        esac
    fi
    echo "ok   $name"
}

# (f) -c for a change other than @.
run "-c a non-@ change is refused" 2 "pushes \`-c\` only for \`@\`" -- -c docs/p
run "--change= a non-@ change is refused" 2 "only for \`@\`" -- --change=docs/p
run "-c a revset holding @ and more is refused" 2 "only for \`@\`" -- -c '@ | docs/p'

# (g) options that would widen or redirect the push.
run "--named n=<non-@> is refused" 2 "only for \`@\`" -- --named n=docs/p --dry-run
run "--named without = is refused" 2 "expected NAME=REVISION" -- --named n
run "-R is refused" 2 "\`-R\` is not an option" -- -R /tmp
run "--config is refused" 2 "\`--config\` is not an option" -- --config a=b
run "--config=… is refused" 2 "\`--config=a=b\` is not an option" -- --config=a=b
run "--at-op is refused" 2 "is not an option" -- --at-op @
run "--ignore-immutable is refused" 2 "is not an option" -- --ignore-immutable
run "-o is refused" 2 "is not an option" -- -o x
run "an attached -bfoo is refused" 2 "use \`-b foo\`" -- -bfoo
run "--remote upstream is refused" 2 "pushes only to origin" -- --remote upstream
run "--bogus is refused" 2 "\`--bogus\` is not an option" -- --bogus
run "a stray positional is refused" 2 "stray argument" -- docs/p
run "a value that looks like an option is refused" 2 "needs a value" -- -b --config
run "a missing value is refused" 2 "needs a value" -- -b

# (h) -b of a bookmark away from @ that holds a paper: the papers check fails.
run "-b of a paper bookmark fails the papers check" 1 ".understand/q.md  (" -- -b docs/p

if [ "$(pushed)" = 0 ]; then
    echo "ok   nothing reached origin from the refused cases"
else
    echo "FAIL something reached origin: $(git --git-dir="$origin" for-each-ref)"
    failures=$((failures + 1))
fi

# The papers check covers only the heads the push names. With the unrelated
# papers bookmark docs/p still there:
run "-b clean --dry-run reaches jj git push" 0 "Dry-run requested" -- -b clean --dry-run
run "--all --dry-run fails the papers check" 1 "[docs/p] add a paper" -- --all --dry-run
run "--tracked --dry-run fails the papers check" 1 "[docs/p] add a paper" -- --tracked --dry-run
run "-b 'glob:*' --dry-run fails the papers check" 1 "[docs/p] add a paper" -- -b 'glob:*' --dry-run
run "-b with a glob metacharacter fails the papers check" 1 "[docs/p] add a paper" -- -b 'c*' --dry-run
run "-r docs/p --dry-run fails the papers check" 1 "[docs/p] add a paper" -- -r docs/p --dry-run
run "a default push (--dry-run alone) fails the papers check" 1 "[docs/p] add a paper" -- --dry-run

# (i) -c @ passes the guard and the checks and reaches `jj git push` against
# origin, docs/p notwithstanding.
run "-c @ --dry-run reaches jj git push" 0 "Dry-run requested" -- -c @ --dry-run
run "--remote=origin --named n=@ --dry-run reaches jj git push" 0 "Dry-run requested" -- --remote=origin --named n=@ --dry-run
run "the cargo gate skipped itself" 0 "skipping fmt/clippy" -- -c @ --dry-run

if [ "$(pushed)" = 0 ]; then
    echo "ok   nothing reached origin (every push was refused, failed or dry-run)"
else
    echo "FAIL something reached origin: $(git --git-dir="$origin" for-each-ref)"
    failures=$((failures + 1))
fi

# (j) A second scratch repo, its own origin: the paper is reachable ONLY from
# a tag (tag `p`), @ is a clean commit on bookmark `clean`. `--all`,
# `--tracked` and jj's default push all cover tags, so each fails the check.
repo="$tmp/tag-repo"
origin="$tmp/tag-origin.git"
git init -q --bare "$origin"
jj git init "$repo" >/dev/null 2>&1
(
    cd "$repo"
    jj git remote add origin "$origin"
    mkdir .understand && echo q >.understand/q.md
    jj commit -m "add a tagged paper" >/dev/null 2>&1
    jj tag set p -r @- >/dev/null 2>&1
    jj new 'root()' -m "clean work" >/dev/null 2>&1
    mkdir scripts
    cp "$script_dir/jj-push.sh" "$script_dir/private-papers-check.sh" scripts/
    jj bookmark create clean -r @ >/dev/null 2>&1
)
run "--all --dry-run fails on a paper only a tag reaches" 1 ".understand/q.md  (" -- --all --dry-run
run "--tracked --dry-run fails on a paper only a tag reaches" 1 ".understand/q.md  (" -- --tracked --dry-run
run "a default push fails on a paper only a tag reaches" 1 ".understand/q.md  (" -- --dry-run

if [ "$(pushed)" = 0 ]; then
    echo "ok   nothing reached the tag repo's origin"
else
    echo "FAIL something reached the tag repo's origin: $(git --git-dir="$origin" for-each-ref)"
    failures=$((failures + 1))
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all jj-push tests passed"
