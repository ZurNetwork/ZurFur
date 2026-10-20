#!/bin/sh
# Fails when the node tree is not committed whole, or when an old-era node file
# is left in it. `nodes check` alone cannot see either: a stray `NODE.json` in a
# tree of `NODE.inf.json` files is passed over with a note on stderr (and one
# at the root turns the whole tree back to the old era), and with no node tree
# at all `nodes check` checks `NODE.refs.json` alone and still passes, so a tree
# that lives on one machine only would keep CI green.
#
# It lists the tracked files the way private-papers-check.sh does (`jj file
# list` in a jj workspace, else `git ls-files`) and fails on:
#   - any `NODE.json` on disk beneath the root, at the root or nested;
#   - `nodes --json check` reporting `"tree": false` (no node tree here);
#   - a `NODE.inf.json` on disk that is not tracked, or a tracked one missing
#     on disk (a `**/*.inf.json` line in a .gitignore keeps the nodes out of
#     git while the tree still checks on the machine that has them);
#   - `NODE.refs.json` not tracked at the root (without it CI checks no ref).
# The disk walk skips the folders nodes always skips: .git, .jj, target and
# node_modules, at any depth.
#
# Exits 1 with one line per finding, 0 with one OK line. Fails closed: exits 2
# when it cannot list the tracked files, the listing is empty, `nodes` or `jq`
# is missing, or `nodes --json check` prints no boolean `tree` field.
set -eu

me=nodes-tree-check

fail_closed() {
    echo "$me: $*" >&2
    exit 2
}

if root=$(jj workspace root 2>/dev/null); then
    cd "$root"
    files=$(jj file list) ||
        fail_closed "\`jj file list\` failed (a stale workspace? try \`jj workspace update-stale\`)"
elif root=$(git rev-parse --show-toplevel 2>/dev/null); then
    cd "$root"
    files=$(git -c core.quotePath=false ls-files) || fail_closed "\`git ls-files\` failed"
else
    fail_closed "not inside a jj workspace or a git checkout"
fi
[ -n "$files" ] || fail_closed "the tracked-file listing is empty"

command -v nodes >/dev/null 2>&1 || fail_closed "\`nodes\` is not on PATH (just nodes-install)"
command -v jq >/dev/null 2>&1 || fail_closed "\`jq\` is not on PATH"

# Errors in the tree are `nodes check`'s own job, so its exit code is not read
# here; only its report's `tree` field is.
report=$(nodes --root "$root" --json check 2>/dev/null) || :
tree=$(printf '%s' "$report" |
    jq -r 'if (.tree | type) == "boolean" then .tree else error("no tree field") end' 2>/dev/null) ||
    fail_closed "\`nodes --json check\` printed no parseable \`tree\` field"
case "$tree" in
    true | false) ;;
    *) fail_closed "\`nodes --json check\` printed no single \`tree\` field" ;;
esac

on_disk=$(find . \( -name .git -o -name .jj -o -name target -o -name node_modules \) -prune -o \
    \( -name NODE.json -o -name NODE.inf.json \) -print) || fail_closed "the walk of the checkout failed"

# Each input line is `D <path>` (on disk) or `T <path>` (tracked); each output
# line is `<group> <finding>`, sorted, then the group is cut off.
findings=$(
    {
        printf '%s\n' "$on_disk" | awk '$0 != "" { sub(/^\.\//, ""); print "D " $0 }'
        printf '%s\n' "$files" | awk '$0 != "" { print "T " $0 }'
        [ "$tree" = true ] || echo "X"
    } | awk '
        function base(p,    n, s) { n = split(p, s, "/"); return s[n] }
        function skipped(p,    n, s, i) {
            n = split(p, s, "/")
            for (i = 1; i < n; i++)
                if (s[i] == ".git" || s[i] == ".jj" || s[i] == "target" || s[i] == "node_modules")
                    return 1
            return 0
        }
        $0 == "X" { print "1 `nodes --json check` reports \"tree\": false: no node tree here, so only NODE.refs.json is checked"; next }
        {
            kind = substr($0, 1, 1)
            p = substr($0, 3)
        }
        kind == "D" {
            if (base(p) == "NODE.json")
                print "2 a stray NODE.json: " p " (nodes 0.7 passes it over; one at the root turns the whole tree back to the old era)"
            else if (base(p) == "NODE.inf.json") { disk[p] = 1; ndisk++ }
            next
        }
        kind == "T" {
            if (p == "NODE.refs.json") refs = 1
            if (base(p) == "NODE.inf.json" && !skipped(p)) { tracked[p] = 1; ntracked++ }
        }
        END {
            differ = 0
            for (p in disk) if (!(p in tracked)) { print "4 not tracked: " p; differ = 1 }
            for (p in tracked) if (!(p in disk)) { print "4 tracked but missing on disk: " p; differ = 1 }
            if (differ)
                printf "3 %d NODE.inf.json tracked, %d on disk (a **/*.inf.json line in a .gitignore keeps nodes out of git)\n", ntracked, ndisk
            if (!refs) print "5 NODE.refs.json is not tracked at the root: CI would check no ref"
        }
    ' | LC_ALL=C sort | cut -d' ' -f2-
) || fail_closed "the listing check itself failed"

if [ -n "$findings" ]; then
    printf '%s\n' "$findings" | awk -v me="$me" '{ print me ": " $0 }'
    exit 1
fi
count=$(printf '%s\n' "$files" | awk '{ n = split($0, s, "/"); if (s[n] == "NODE.inf.json") c++ } END { print c + 0 }')
echo "$me: OK, a committed node tree: $count NODE.inf.json and NODE.refs.json tracked, no NODE.json"
