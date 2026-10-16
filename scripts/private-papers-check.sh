#!/bin/sh
# Fails when a tracked path looks like a private working paper. ZurFur is a
# public repo; working papers (rulings, the queue, handoffs, reviews, retros,
# release documents) live on the private design corpus's `understand` branch,
# never here (the Engineer's ruling, 2026-10-16).
#
#   private-papers-check.sh           checks the tracked files: `jj file list`
#                                     in a jj workspace (the working copy, @),
#                                     else `git ls-files`
#   private-papers-check.sh --stdin   checks one repo-relative path per line
#
# A path is a working paper when it has a `.understand/`, `.teams/`,
# `releases/` or `retros/` folder at any depth, a file name starting
# `rulings-` or exactly `queue.md`, or lies in a release-documents folder
# directly under the root docs/: `docs/<version>/…`, where <version> is
# `0.1`, `v0.2` or `{stage}-v{version}` such as `pre-alpha-v0.2.0`.
#
# Exits 1 and lists each offending path with the rule it broke; exits 0 with
# one OK line when clean. Fails closed: exits 2 when it cannot list the
# tracked files, or the listing comes back empty.
set -eu

mode="${1:-tracked}"
case "$mode" in
    tracked | --stdin) ;;
    *)
        echo "usage: private-papers-check.sh [--stdin]" >&2
        exit 2
        ;;
esac

# check reads paths on stdin, prints each hit, and exits 1 on any hit. A path
# git had to quote ("…") is unquoted first; its escapes never hold a `/`.
check() {
    awk '
        function hit(rule) { printf "  %s  (%s)\n", $0, rule; found = 1 }
        $0 == "" { next }
        {
            path = $0
            if (path ~ /^".*"$/) path = substr(path, 2, length(path) - 2)
            n = split(path, seg, "/")
            name = seg[n]
            for (i = 1; i < n; i++) {
                if (seg[i] == ".understand" || seg[i] == ".teams" ||
                    seg[i] == "releases" || seg[i] == "retros") {
                    hit("a " seg[i] "/ folder"); next
                }
            }
            if (index(name, "rulings-") == 1) { hit("a rulings-* file"); next }
            if (name == "queue.md") { hit("a queue.md file"); next }
            if (n >= 3 && seg[1] == "docs" &&
                seg[2] ~ /^([a-z]+(-[a-z]+)*-)?v?[0-9]+(\.[0-9]+)*$/) {
                hit("a release-documents folder under docs/"); next
            }
        }
        END { exit found }
    '
}

if [ "$mode" = --stdin ]; then
    hits=$(check) && status=0 || status=$?
else
    if root=$(jj workspace root 2>/dev/null); then
        cd "$root"
        files=$(jj file list) || { echo "private-papers-check: \`jj file list\` failed" >&2; exit 2; }
    elif root=$(git rev-parse --show-toplevel 2>/dev/null); then
        cd "$root"
        files=$(git -c core.quotePath=false ls-files) || { echo "private-papers-check: \`git ls-files\` failed" >&2; exit 2; }
    else
        echo "private-papers-check: not inside a jj workspace or a git checkout" >&2
        exit 2
    fi
    if [ -z "$files" ]; then
        echo "private-papers-check: the tracked-file listing is empty" >&2
        exit 2
    fi
    hits=$(printf '%s\n' "$files" | check) && status=0 || status=$?
fi

case "$status" in
    0) echo "private-papers-check: OK, no working papers tracked" ;;
    1)
        echo "private-papers-check: these tracked paths are private working papers:"
        printf '%s\n' "$hits"
        echo "private-papers-check: working papers belong on the design corpus's \`understand\` branch, never in this public repo (the Engineer's ruling, 2026-10-16)."
        exit 1
        ;;
    *)
        echo "private-papers-check: the path check itself failed" >&2
        exit 2
        ;;
esac
