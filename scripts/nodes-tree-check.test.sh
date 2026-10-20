#!/bin/sh
# Tests for scripts/nodes-tree-check.sh. Run directly, or via `just scripts-test`.
# A `nodes` stub first on PATH prints $NODES_STUB_CHECK for `--json check`, so
# no real `nodes` is needed; the jj fixtures skip themselves without jj.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/nodes-tree-check.sh"
sh_bin="$(command -v sh)"
failures=0

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_CEILING_DIRECTORIES="$tmp"
printf '[user]\nname = "fixture"\nemail = "fixture@example.invalid"\n' >"$tmp/jj-config.toml"
export JJ_CONFIG="$tmp/jj-config.toml"

mkdir "$tmp/stub"
cat >"$tmp/stub/nodes" <<'STUB'
#!/bin/sh
for arg in "$@"; do
    if [ "$arg" = check ]; then
        printf '%s\n' "${NODES_STUB_CHECK-}"
        exit "${NODES_STUB_EXIT:-0}"
    fi
done
exit 1
STUB
chmod +x "$tmp/stub/nodes"
stub_path="$tmp/stub:$PATH"
tree_true='{"nodes":2,"mounts":0,"refs":1,"tree":true,"errors":0,"warnings":0,"findings":[]}'
tree_false='{"nodes":0,"mounts":0,"refs":1,"tree":false,"errors":0,"warnings":0,"findings":[]}'

# toolbox <dir> <tool>...: a PATH directory holding only the named tools (each
# one found on PATH is linked; a missing one is left out).
toolbox() {
    dir="$1"
    shift
    mkdir -p "$dir"
    for tool in "$@"; do
        found="$(command -v "$tool" 2>/dev/null)" || continue
        ln -sf "$found" "$dir/$tool"
    done
}
tools="awk cat cut find git jj mktemp rm sort"
toolbox "$tmp/no-nodes" $tools jq
toolbox "$tmp/no-jq" $tools
ln -sf "$tmp/stub/nodes" "$tmp/no-jq/nodes"

# failing <dir> <tool> [needle]: a PATH directory with the stub nodes and every
# tool, where <tool> exits 2 when its arguments hold [needle] (default: always)
# and otherwise runs the real one.
failing() {
    toolbox "$1" $tools jq
    ln -sf "$tmp/stub/nodes" "$1/nodes"
    real="$(command -v "$2")"
    rm -f "$1/$2"
    {
        echo '#!/bin/sh'
        printf 'case "$*" in *%s*) exit 2 ;; esac\n' "${3-}"
        printf 'exec %s "$@"\n' "$real"
    } >"$1/$2"
    chmod +x "$1/$2"
}
# bites <tool>: whether this shell runs a PATH stub of <tool> at all (busybox
# sh runs its own awk, sort and cut applets ahead of PATH).
bites() {
    mkdir -p "$tmp/probe-$1"
    printf '#!/bin/sh\nexit 3\n' >"$tmp/probe-$1/$1"
    chmod +x "$tmp/probe-$1/$1"
    PATH="$tmp/probe-$1" "$sh_bin" -c "$1" </dev/null >/dev/null 2>&1 && return 1
    [ $? = 3 ]
}
failing "$tmp/awk-fails" awk
failing "$tmp/findings-awk-fails" awk 'countfile='
failing "$tmp/print-awk-fails" awk 'me='
failing "$tmp/sort-fails" sort
failing "$tmp/cut-fails" cut
failing "$tmp/find-fails" find

# tree <dir>: the files of a small committed node tree.
tree() {
    mkdir -p "$1/docs"
    echo '{"path":"."}' >"$1/NODE.inf.json"
    echo '{"path":"docs"}' >"$1/docs/NODE.inf.json"
    echo '{"docs":[{"page":1,"title":"x","governs":"x"}]}' >"$1/NODE.refs.json"
    echo readme >"$1/README.md"
}

# git_fixture <name>: a git checkout of the tree, every file in the index;
# prints its path.
git_fixture() {
    dir="$tmp/$1"
    git init -q "$dir"
    tree "$dir"
    git -C "$dir" add -A
    printf '%s\n' "$dir"
}

# jj_fixture <name>: a jj workspace of the tree, snapshotted, not colocated
# (like Zurfur's workspaces, so git cannot list it); prints its path.
jj_fixture() {
    dir="$tmp/$1"
    jj git init --no-colocate "$dir" >/dev/null 2>&1 || jj git init "$dir" >/dev/null 2>&1
    tree "$dir"
    (cd "$dir" && jj describe -m tree >/dev/null 2>&1)
    printf '%s\n' "$dir"
}

# expect <name> <dir> <expected exit> [text the output must hold]...
# Runs the guard in <dir>; the stub prints $check (default: "tree": true) and
# exits $stub_exit (default 0), and PATH is $case_path (default: the stub
# first). A case sets these before its call and unsets them after.
expect() {
    name="$1"
    dir="$2"
    want="$3"
    shift 3
    out="$(cd "$dir" && NODES_STUB_CHECK="${check-$tree_true}" NODES_STUB_EXIT="${stub_exit:-0}" \
        PATH="${case_path:-$stub_path}" "$sh_bin" "$under_test" 2>&1)" && got=0 || got=$?
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

r="$(git_fixture clean)"
expect "git: a clean committed tree passes" "$r" 0 "OK, a committed node tree: 2 NODE.inf.json"

r="$(git_fixture root-stray)"
cp "$r/NODE.inf.json" "$r/NODE.json"
expect "git: a root NODE.json fails" "$r" 1 "a stray NODE.json: NODE.json "

r="$(git_fixture nested-stray)"
cp "$r/docs/NODE.inf.json" "$r/docs/NODE.json"
expect "git: a nested NODE.json fails" "$r" 1 "a stray NODE.json: docs/NODE.json "

r="$(git_fixture dotted-name)"
mkdir -p "$r/docs.v2"
cp "$r/docs/NODE.inf.json" "$r/docs.v2/NODE.json"
expect "git: a NODE.json in a folder with a dot inside its name fails" "$r" 1 "a stray NODE.json: docs.v2/NODE.json "

r="$(git_fixture hidden-ignored)"
echo '.claude/worktrees/' >"$r/.gitignore"
git -C "$r" add .gitignore
mkdir -p "$r/.claude/worktrees/x/docs"
cp "$r/NODE.inf.json" "$r/.claude/worktrees/x/NODE.inf.json"
cp "$r/docs/NODE.inf.json" "$r/.claude/worktrees/x/docs/NODE.json"
expect "git: node files in an ignored hidden folder pass" "$r" 0 "OK, a committed node tree: 2 NODE.inf.json"

r="$(git_fixture hidden-tracked)"
mkdir -p "$r/.github/sub"
echo '{}' >"$r/.github/NODE.json"
echo '{}' >"$r/.github/sub/NODE.inf.json"
git -C "$r" add -A
expect "git: tracked node files in a hidden folder pass" "$r" 0 "OK, a committed node tree: 2 NODE.inf.json"

r="$(git_fixture ignored-not-hidden)"
echo 'build/' >"$r/.gitignore"
git -C "$r" add .gitignore
mkdir -p "$r/build/x"
cp "$r/NODE.inf.json" "$r/build/x/NODE.json"
expect "git: a NODE.json in an ignored folder that is not hidden still fails (no ignore file is read)" \
    "$r" 1 "a stray NODE.json: build/x/NODE.json "

r="$(git_fixture build-folders)"
mkdir -p "$r/target/debug" "$r/web/node_modules/pkg"
echo '{}' >"$r/target/debug/NODE.json"
echo '{}' >"$r/target/debug/NODE.inf.json"
echo '{}' >"$r/web/node_modules/pkg/NODE.json"
echo '{}' >"$r/web/node_modules/pkg/NODE.inf.json"
expect "git: node files under target/ and node_modules/ pass" "$r" 0 "OK"

r="$(git_fixture no-tree)"
check="$tree_false"
expect "git: \"tree\": false fails" "$r" 1 '"tree": false'
unset check

r="$(git_fixture inf-ignored)"
echo '**/*.inf.json' >"$r/.gitignore"
git -C "$r" add .gitignore
git -C "$r" rm -q --cached docs/NODE.inf.json
expect "git: an ignored, untracked NODE.inf.json fails on the count" "$r" 1 \
    "1 NODE.inf.json tracked, 2 on disk" "not tracked: docs/NODE.inf.json"

r="$(git_fixture inf-missing)"
rm "$r/docs/NODE.inf.json"
expect "git: a tracked NODE.inf.json missing on disk fails" "$r" 1 \
    "tracked but missing on disk: docs/NODE.inf.json"

r="$(git_fixture refs-untracked)"
git -C "$r" rm -q --cached NODE.refs.json
expect "git: an untracked NODE.refs.json fails" "$r" 1 "NODE.refs.json is not tracked at the root"

r="$(git_fixture several)"
cp "$r/NODE.inf.json" "$r/NODE.json"
git -C "$r" rm -q --cached NODE.refs.json
out="$(cd "$r" && NODES_STUB_CHECK="$tree_false" PATH="$stub_path" "$sh_bin" "$under_test" 2>&1)" && got=0 || got=$?
lines="$(printf '%s\n' "$out" | awk 'END { print NR }')"
if [ "$got" = 1 ] && [ "$lines" = 3 ]; then
    echo "ok   git: three findings print three lines"
else
    echo "FAIL git: three findings: expected exit 1 and 3 lines, got $got and $lines: $out"
    failures=$((failures + 1))
fi

r="$(git_fixture tools)"
case_path="$tmp/no-nodes"
expect "no nodes on PATH fails closed" "$r" 2 "\`nodes\` is not on PATH"
unset case_path
case_path="$tmp/no-jq"
expect "no jq on PATH fails closed" "$r" 2 "\`jq\` is not on PATH"
unset case_path
check="garbage, not JSON"
expect "garbage from nodes --json check fails closed" "$r" 2 "no parseable"
unset check
check='{"nodes":2,"errors":0}'
expect "a report with no tree field fails closed" "$r" 2 "no parseable"
unset check
check=''
expect "an empty report fails closed" "$r" 2 "no single"
unset check
check="$tree_true$tree_true"
expect "two reports fail closed" "$r" 2 "no single"
unset check
stub_exit=1
expect "a failing nodes check is still read for its tree field" "$r" 0 "OK"
unset stub_exit
if bites awk; then
    case_path="$tmp/awk-fails"
    expect "a failing awk fails closed" "$r" 2 "listing the node files on disk failed"
    case_path="$tmp/findings-awk-fails"
    expect "a failing findings awk fails closed" "$r" 2 "the listing check itself failed"
    unset case_path
    r2="$(git_fixture print-fails)"
    cp "$r2/NODE.inf.json" "$r2/NODE.json"
    case_path="$tmp/print-awk-fails"
    expect "a failing awk printing the findings fails closed" "$r2" 2 "printing the findings failed"
    unset case_path
else
    echo "skip (this sh runs its own awk): the failing-awk cases"
fi
if bites sort; then
    case_path="$tmp/sort-fails"
    expect "a failing sort fails closed" "$r" 2 "sorting the findings failed"
    unset case_path
else
    echo "skip (this sh runs its own sort): the failing-sort case"
fi
if bites cut; then
    case_path="$tmp/cut-fails"
    expect "a failing cut fails closed" "$r" 2 "cutting the findings failed"
    unset case_path
else
    echo "skip (this sh runs its own cut): the failing-cut case"
fi
if bites find; then
    case_path="$tmp/find-fails"
    expect "a failing find fails closed" "$r" 2 "the walk of the checkout failed"
    unset case_path
else
    echo "skip (this sh runs its own find): the failing-find case"
fi

mkdir "$tmp/empty"
git init -q "$tmp/empty"
expect "an empty listing fails closed" "$tmp/empty" 2 "the tracked-file listing is empty"

mkdir "$tmp/outside"
expect "outside any checkout fails closed" "$tmp/outside" 2 "not inside a jj workspace or a git checkout"

if command -v jj >/dev/null 2>&1; then
    r="$(jj_fixture jj-clean)"
    expect "jj: a clean committed tree passes" "$r" 0 "OK, a committed node tree: 2 NODE.inf.json"

    r="$(jj_fixture jj-root-stray)"
    cp "$r/NODE.inf.json" "$r/NODE.json"
    expect "jj: a root NODE.json fails" "$r" 1 "a stray NODE.json: NODE.json "

    r="$(jj_fixture jj-nested-stray)"
    cp "$r/docs/NODE.inf.json" "$r/docs/NODE.json"
    expect "jj: a nested NODE.json fails" "$r" 1 "a stray NODE.json: docs/NODE.json "

    r="$(jj_fixture jj-hidden-ignored)"
    echo '.claude/worktrees/' >"$r/.gitignore"
    mkdir -p "$r/.claude/worktrees/x/docs"
    cp "$r/NODE.inf.json" "$r/.claude/worktrees/x/NODE.inf.json"
    cp "$r/docs/NODE.inf.json" "$r/.claude/worktrees/x/docs/NODE.json"
    expect "jj: node files in an ignored hidden folder pass" "$r" 0 "OK, a committed node tree: 2 NODE.inf.json"

    r="$(jj_fixture jj-inf-untracked)"
    echo '**/*.inf.json' >"$r/.gitignore"
    (cd "$r" && jj file untrack docs/NODE.inf.json >/dev/null 2>&1)
    expect "jj: an ignored, untracked NODE.inf.json fails on the count" "$r" 1 \
        "1 NODE.inf.json tracked, 2 on disk" "not tracked: docs/NODE.inf.json"

    r="$(jj_fixture jj-refs-untracked)"
    echo 'NODE.refs.json' >"$r/.gitignore"
    (cd "$r" && jj file untrack NODE.refs.json >/dev/null 2>&1)
    expect "jj: an untracked NODE.refs.json fails" "$r" 1 "NODE.refs.json is not tracked at the root"
else
    echo "skip (no jj): the jj fixtures"
fi

# The wiring: `just nodes-check` runs the guard after `nodes check`. The body
# of its recipe is its indented lines up to the next recipe, each trimmed.
nodes_check_body="$(awk '
    /^nodes-check:/ { inside = 1; next }
    inside && /^[^[:space:]]/ { inside = 0 }
    inside { sub(/^[[:space:]]+/, ""); sub(/[[:space:]]+$/, ""); print }
' "$script_dir/../Justfile")"
if printf '%s\n' "$nodes_check_body" | awk '
    $0 == "nodes check --ref-index docs/design-index.md" && !check { check = NR }
    $0 == "sh scripts/nodes-tree-check.sh" && check { guard = 1 }
    END { exit !guard }
'; then
    echo "ok   just nodes-check runs the guard after nodes check"
else
    echo "FAIL just nodes-check is not wired to the guard: no line 'sh scripts/nodes-tree-check.sh' after 'nodes check --ref-index docs/design-index.md' in its recipe: $nodes_check_body"
    failures=$((failures + 1))
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all nodes-tree-check tests passed"
