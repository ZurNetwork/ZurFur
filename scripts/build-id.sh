#!/bin/sh
# build-id.sh: prints one line naming the build of this checkout, for
# ZURFUR_BUILD_ID. jj first: the working copy's parent commit (12 characters,
# merge parents joined by "+"). Then git: HEAD. Either way " (local changes)"
# follows when the working copy differs from that commit. Then "unknown".
# Quiet (each tool's stderr is dropped) and never fails. The dev recipes stamp
# it once, when they start. Tested by build-id.test.sh.
set -u

# The id must fit a BuildId: one line, at most 64 bytes.
fits() {
    case "$1" in
        "" | *"
"*) return 1 ;;
    esac
    [ "${#1}" -le 64 ]
}

case "$0" in
    */*) script_dir="${0%/*}" ;;
    *) script_dir=. ;;
esac
cd "$script_dir/.." 2>/dev/null || {
    echo unknown
    exit 0
}

if id="$(jj --quiet log --no-graph -r @ \
    -T 'parents.map(|c| c.commit_id().short(12)).join("+") ++ if(empty, "", " (local changes)")' \
    2>/dev/null)" && fits "$id"; then
    echo "$id"
    exit 0
fi

if commit="$(git rev-parse --short=12 HEAD 2>/dev/null)" &&
    changes="$(git status --porcelain 2>/dev/null)"; then
    id="$commit"
    if [ -n "$changes" ]; then
        id="$commit (local changes)"
    fi
    if fits "$id"; then
        echo "$id"
        exit 0
    fi
fi

echo unknown
