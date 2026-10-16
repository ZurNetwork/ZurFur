#!/bin/sh
# Fails when a path looks like a private working paper. ZurFur is a public
# repo; working papers (rulings, the queue, handoffs, reviews, retros, release
# documents) live on the private design corpus's `understand` branch, never
# here (the Engineer's ruling, 2026-10-16).
#
#   private-papers-check.sh             checks the tracked files: `jj file list`
#                                       in a jj workspace (the working copy, @),
#                                       else `git ls-files`
#   private-papers-check.sh --stdin     checks one repo-relative path per line
#   private-papers-check.sh --unpushed [HEADS]
#                                       (jj only) checks what each commit not
#                                       on origin that HEADS reaches adds or
#                                       changes; HEADS is a revset, by default
#                                       `bookmarks() | tags() | @`. The range is
#                                       remote_bookmarks(remote=exact:"origin")..(HEADS)
#                                       Added, modified, renamed and copied
#                                       paths are checked (a rename on its
#                                       target); removed ones are not, so
#                                       deleting an already-public paper stays
#                                       pushable, and a file unchanged since a
#                                       commit origin has sends nothing new. A
#                                       paper added in one commit and deleted
#                                       in the next is still caught at the
#                                       commit that added it. `jj push` runs
#                                       this mode with the heads it pushes.
#
# A path is a working paper when, ignoring case, it has a `.understand/`,
# `.teams/`, `releases/` or `retros/` folder at any depth, a file name
# starting `rulings-` or `handoff-`, or exactly `queue.md` or
# `engineer-queue.md`, or lies in a release-documents folder directly under
# the root docs/: `docs/<version>/…`, where <version> is `0.1`, `v0.2`,
# `{stage}-v{version}` such as `pre-alpha-v0.2.0`, or any of those with a
# suffix such as `-rc1`.
#
# Exits 1 and lists each offending path with the rule it broke (in --unpushed
# mode, under the commit that adds or changes it); exits 0 with one OK line
# when clean. Fails closed: exits 2 when it cannot list the tracked files, the
# tracked listing comes back empty, or the jj call in --unpushed mode fails or
# prints a line it does not recognise.
set -eu

usage() {
    echo "usage: private-papers-check.sh [--stdin | --unpushed [HEADS]]" >&2
    exit 2
}
mode="${1:-tracked}"
heads='bookmarks() | tags() | @'
case "$mode" in
    tracked | --stdin) [ "$#" -le 1 ] || usage ;;
    --unpushed)
        [ "$#" -le 2 ] || usage
        if [ "$#" = 2 ]; then
            [ -n "$2" ] || usage
            heads="$2"
        fi
        ;;
    *) usage ;;
esac

stale_hint="(a stale workspace? try \`jj workspace update-stale\`)"
understand_line="private-papers-check: working papers belong on the design corpus's \`understand\` branch, never in this public repo (the Engineer's ruling, 2026-10-16)."

# check reads paths on stdin, prints each hit, and exits 1 on any hit. A path
# git had to quote ("…") is unquoted first; its escapes never hold a `/`. The
# rules match the lower-cased path; a hit prints the path as given.
check() {
    awk '
        function hit(rule) { printf "  %s  (%s)\n", $0, rule; found = 1 }
        $0 == "" { next }
        {
            path = $0
            if (path ~ /^".*"$/) path = substr(path, 2, length(path) - 2)
            n = split(tolower(path), seg, "/")
            name = seg[n]
            for (i = 1; i < n; i++) {
                if (seg[i] == ".understand" || seg[i] == ".teams" ||
                    seg[i] == "releases" || seg[i] == "retros") {
                    hit("a " seg[i] "/ folder"); next
                }
            }
            if (index(name, "rulings-") == 1) { hit("a rulings-* file"); next }
            if (index(name, "handoff-") == 1) { hit("a handoff-* file"); next }
            if (name == "queue.md") { hit("a queue.md file"); next }
            if (name == "engineer-queue.md") { hit("an engineer-queue.md file"); next }
            if (n >= 3 && seg[1] == "docs" &&
                seg[2] ~ /^([a-z]+(-[a-z]+)*-)?v?[0-9]+(\.[0-9]+)*(-[a-z0-9.]+)?$/) {
                hit("a release-documents folder under docs/"); next
            }
        }
        END { exit found }
    '
}

# unpushed checks what each commit in the range adds or changes: one `jj log`
# call (which snapshots the working copy first, so a new file in @ that jj
# auto-tracks is included) prints, per commit, a header line and one
# `<status>\t<json path>` line per changed file. `removed` entries are
# skipped; every other path (a rename's or copy's target) goes through check.
# Any line it does not recognise fails closed.
unpushed() {
    if ! root=$(jj workspace root 2>/dev/null); then
        echo "private-papers-check: --unpushed needs a jj workspace" >&2
        exit 2
    fi
    cd "$root"
    range="remote_bookmarks(remote=exact:\"origin\")..($heads)"
    template='"commit\t" ++ change_id.short() ++ " " ++ commit_id.short()
        ++ if(bookmarks, " [" ++ bookmarks.join(" ") ++ "]")
        ++ " " ++ if(description, description.first_line(), "(no description)") ++ "\n"
        ++ self.diff().files().map(|e| e.status() ++ "\t" ++ json(e.path()) ++ "\n").join("")'
    listing=$(jj log --no-graph -r "$range" -T "$template") || {
        echo "private-papers-check: \`jj log\` over the unpushed range failed $stale_hint" >&2
        exit 2
    }
    tab=$(printf '\t')
    checked=0
    found=0
    report=""
    label=""
    paths=""
    # flush checks the paths gathered for the commit named by $label.
    flush() {
        [ -n "$label" ] || return 0
        hits=$(printf '%s' "$paths" | check) && status=0 || status=$?
        case "$status" in
            0) ;;
            1)
                found=1
                report="$report  commit $label
$(printf '%s\n' "$hits" | awk '{ print "  " $0 }')
"
                ;;
            *)
                echo "private-papers-check: the path check itself failed" >&2
                exit 2
                ;;
        esac
    }
    while IFS= read -r line; do
        case "$line" in
            "commit$tab"*)
                flush
                checked=$((checked + 1))
                label=${line#"commit$tab"}
                paths=""
                ;;
            "removed$tab"*) ;;
            "added$tab\""* | "modified$tab\""* | "renamed$tab\""* | "copied$tab\""*)
                path=${line#*"$tab"}
                # A JSON string without escapes is the path itself; one with
                # escapes stays quoted, which check reads as a quoted path.
                case "$path" in
                    *\\*) ;;
                    *) path=${path#\"} && path=${path%\"} ;;
                esac
                paths="$paths$path
"
                ;;
            "") ;;
            *)
                echo "private-papers-check: unexpected line from \`jj log\`: $line" >&2
                exit 2
                ;;
        esac
    done <<LISTING
$listing
LISTING
    flush
    if [ "$found" = 1 ]; then
        echo "private-papers-check: these unpushed commits add or change private working papers:"
        printf '%s' "$report"
        echo "$understand_line"
        exit 1
    fi
    echo "private-papers-check: OK, no working papers added or changed in the $checked unpushed commit(s) checked"
    exit 0
}

if [ "$mode" = --unpushed ]; then
    unpushed
elif [ "$mode" = --stdin ]; then
    hits=$(check) && status=0 || status=$?
else
    if root=$(jj workspace root 2>/dev/null); then
        cd "$root"
        files=$(jj file list) || { echo "private-papers-check: \`jj file list\` failed $stale_hint" >&2; exit 2; }
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
        echo "$understand_line"
        exit 1
        ;;
    *)
        echo "private-papers-check: the path check itself failed" >&2
        exit 2
        ;;
esac
