#!/usr/bin/env bash
# Reviews a GitHub PR locally in the colocated checkout, so VS Code's Source
# Control and rust-analyzer show the whole PR as uncommitted changes.
#
#   engineer-tools/review-pr.sh --start --pr=<N>   start, or resume a paused review
#   engineer-tools/review-pr.sh --pause [--pr=<N>] keep the review and its notes, go back
#   engineer-tools/review-pr.sh --stop [--pr=<N>] [--yes]
#                                                  drop the review, go back
#   engineer-tools/review-pr.sh                    list the active and paused reviews
#
# A review is a scratch jj change: a child of the point where the PR left its
# base branch, holding the PR head's files. Notes typed into those files
# (`// REVIEW:` lines, say) are the change's difference from the PR head;
# --pause keeps them and --start brings them back. "Back" is the change that
# was open before --start. The scratch change carries no bookmark, so it can't
# be pushed, and this script never pushes.
#
# Env: REVIEW_PR_ROOT (the colocated checkout; default ~/code/zurfur),
#      XDG_STATE_HOME (state goes to $XDG_STATE_HOME/zurfur/reviews/;
#        default ~/.local/state).
# Exit: 0 done, 1 refused (with the reason), 2 usage error.
set -euo pipefail

# usage prints this file's header comment.
usage() {
    awk 'NR > 1 && /^#/ { sub(/^# ?/, ""); print; next } NR > 1 { exit }' "$0"
    exit 0
}

die() {
    echo "review-pr: $*" >&2
    exit 1
}

usage_error() {
    echo "review-pr: $*" >&2
    echo "run with --help for usage" >&2
    exit 2
}

action=""
pr=""
yes=0
set_action() {
    if [ -n "$action" ] && [ "$action" != "$1" ]; then
        usage_error "--$action and --$1 can't be used together; pick one of --start, --pause, --stop"
    fi
    action="$1"
}
while [ $# -gt 0 ]; do
    case "$1" in
        --start) set_action start ;;
        --pause) set_action pause ;;
        --stop) set_action stop ;;
        --pr=*) pr="${1#--pr=}" ;;
        --pr)
            [ $# -ge 2 ] || usage_error "--pr needs a PR number"
            pr="$2"
            shift
            ;;
        --yes) yes=1 ;;
        -h | --help) usage ;;
        *) usage_error "unknown argument: $1" ;;
    esac
    shift
done
if [ -n "$pr" ] && ! [[ $pr =~ ^[0-9]+$ ]]; then
    usage_error "--pr wants a PR number, got '$pr'"
fi
if [ "$action" = start ] && [ -z "$pr" ]; then
    usage_error "--start needs --pr=<N>"
fi
if [ "$yes" = 1 ] && [ "$action" != stop ]; then
    usage_error "--yes only goes with --stop"
fi

# Only in the colocated checkout: the jj workspaces have no .git, so VS Code
# can't show the review there.
expected_root="$(cd "${REVIEW_PR_ROOT:-$HOME/code/zurfur}" 2>/dev/null && pwd -P)" ||
    die "the checkout ${REVIEW_PR_ROOT:-$HOME/code/zurfur} doesn't exist"
root="$(jj workspace root 2>/dev/null)" || die "not in a jj checkout; run this in $expected_root"
root="$(cd "$root" && pwd -P)"
if [ "$root" != "$expected_root" ]; then
    die "run this in $expected_root, the colocated checkout (this is $root)"
fi
if [ ! -d "$root/.git" ]; then
    die "$root has no .git, so VS Code can't show the review; use the colocated checkout"
fi
cd "$root"

state_dir="${XDG_STATE_HOME:-$HOME/.local/state}/zurfur/reviews"
mkdir -p "$state_dir"

# jj_one <revset> <template> — one value from jj log, or nothing.
jj_one() {
    jj log --no-graph --ignore-working-copy -r "$1" -T "$2" 2>/dev/null || true
}

# The working copy's change id, after jj snapshots any edits.
current_change() {
    jj log --no-graph -r @ -T 'change_id' 2>/dev/null
}

refuse_if_conflicted() {
    if [ "$(jj log --no-graph -r @ -T 'if(conflict, "yes")')" = yes ]; then
        die "the working copy has conflicts; resolve them first"
    fi
}

# state_get <pr> <key>
state_get() {
    awk -F= -v key="$2" '$1 == key { sub(/^[^=]*=/, ""); print; exit }' "$state_dir/$1.state"
}

# state_write <pr> key=value…
state_write() {
    local file="$state_dir/$1.state"
    shift
    printf '%s\n' "$@" >"$file.tmp"
    mv "$file.tmp" "$file"
}

# state_set <pr> <key> <value>
state_set() {
    local file="$state_dir/$1.state"
    awk -F= -v key="$2" -v value="$3" '$1 == key { print key "=" value; next } { print }' "$file" >"$file.tmp"
    mv "$file.tmp" "$file"
}

reviews_with_status() {
    local f
    for f in "$state_dir"/*.state; do
        [ -e "$f" ] || continue
        if [ -z "${1:-}" ] || [ "$(awk -F= '$1 == "status" { print $2 }' "$f")" = "$1" ]; then
            basename "$f" .state
        fi
    done
}

# go_back <pr> — returns to the change that was open before --start.
go_back() {
    local back parents
    back="$(state_get "$1" return_change)"
    parents="$(state_get "$1" return_parents)"
    if [ -n "$(jj_one "$back" 'change_id')" ]; then
        jj edit "$back" >/dev/null 2>&1 || die "couldn't go back to $back"
        echo "Back on $back."
    else
        # The change was empty, so jj dropped it when the review started; a
        # new empty change on the same parents is the same place.
        # shellcheck disable=SC2086 # parent change ids, space-separated
        jj new $parents >/dev/null 2>&1 || die "couldn't go back to a new change on $parents"
        echo "Back on a new change on $parents ($back was empty, so jj had dropped it)."
    fi
}

# pick_pr <status> — the PR to act on: --pr, else the only review in <status>.
pick_pr() {
    local found
    if [ -n "$pr" ]; then
        [ -f "$state_dir/$pr.state" ] || die "no review of #$pr; see the list with no arguments"
        echo "$pr"
        return
    fi
    found="$(reviews_with_status "$1")"
    case "$(printf '%s' "$found" | grep -c .)" in
        1) echo "$found" ;;
        0) die "no $1 review; pass --pr=<N>" ;;
        *) die "several $1 reviews ($(printf '%s' "$found" | tr '\n' ',')); pass --pr=<N>" ;;
    esac
}

list_reviews() {
    local n any=0
    for n in $(reviews_with_status); do
        any=1
        printf '#%s %s  change %s  head %s  started %s\n' "$n" \
            "$(state_get "$n" status)" "$(state_get "$n" change)" \
            "$(state_get "$n" head | cut -c1-12)" "$(state_get "$n" started)"
    done
    [ "$any" = 1 ] || echo "No reviews. Start one with --start --pr=<N>."
}

start_review() {
    local active answer base head fork review now back parents old_head
    refuse_if_conflicted
    active="$(reviews_with_status active)"
    if [ -n "$active" ] && [ "$active" != "$pr" ]; then
        die "review #$active is active; --pause or --stop it first"
    fi

    command -v gh >/dev/null 2>&1 || die "gh isn't installed"
    if ! answer="$(gh pr view "$pr" --json baseRefName,headRefOid --jq '.baseRefName + " " + .headRefOid' 2>/dev/null)"; then
        die "can't read PR #$pr from GitHub"
    fi
    read -r base head <<<"$answer"
    [[ $head =~ ^[0-9a-f]{40}$ ]] && [ -n "$base" ] || die "GitHub's answer for #$pr doesn't parse: '$answer'"
    jj git fetch >/dev/null 2>&1 || die "jj git fetch failed"
    [ -n "$(jj_one "$head" 'commit_id')" ] ||
        die "PR #$pr's head $head isn't on origin after the fetch (a fork's PR?)"
    [ -n "$(jj_one "$base@origin" 'commit_id')" ] || die "the base branch $base@origin isn't known after the fetch"

    if [ -f "$state_dir/$pr.state" ]; then
        review="$(state_get "$pr" change)"
        [ -n "$(jj_one "$review" 'change_id')" ] ||
            die "the review change $review of #$pr is gone; remove $state_dir/$pr.state to start over"
        now="$(current_change)"
        if [ "$now" = "$review" ]; then
            echo "Review of #$pr is already open."
        else
            if [ "$(state_get "$pr" status)" = paused ]; then
                state_set "$pr" return_change "$now"
                state_set "$pr" return_parents "$(jj_one "$now-" 'change_id ++ " "' | xargs)"
            fi
            jj edit "$review" >/dev/null 2>&1 || die "couldn't open the review change $review"
            state_set "$pr" status active
            echo "Resumed the review of #$pr (change $review), notes kept."
        fi
        old_head="$(state_get "$pr" head)"
        if [ "$old_head" != "$head" ]; then
            echo "warning: #$pr's head moved since this review started: ${old_head:0:12}..${head:0:12}." >&2
            echo "warning: the files still show ${old_head:0:12}; see what changed with: jj diff --from ${old_head:0:12} --to ${head:0:12}" >&2
        fi
    else
        back="$(current_change)"
        parents="$(jj_one "@-" 'change_id ++ " "' | xargs)"
        # Where the PR left its base: the diff then matches GitHub's.
        fork="$(jj_one "fork_point($base@origin | $head)" 'commit_id')"
        [ -n "$fork" ] || die "no common ancestor of $base@origin and $head"
        jj new "$fork" -m "review #$pr (scratch, never push)" >/dev/null 2>&1 || die "couldn't create the review change"
        review="$(current_change)"
        jj restore --from "$head" >/dev/null 2>&1 || die "couldn't load #$pr's files into $review"
        state_write "$pr" "pr=$pr" "status=active" "change=$review" "return_change=$back" \
            "return_parents=$parents" "base=$base" "fork=$fork" "head=$head" \
            "started=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        echo "Review of #$pr (into $base) open as change $review; you were on $back."
    fi
    echo "Next: code $root"
}

pause_review() {
    local n
    n="$(pick_pr active)"
    [ "$(state_get "$n" status)" = active ] || die "the review of #$n is already paused"
    refuse_if_conflicted
    current_change >/dev/null
    go_back "$n"
    state_set "$n" status paused
    echo "Paused the review of #$n; --start --pr=$n resumes it with its notes."
}

stop_review() {
    local n review head notes
    n="$(pick_pr active)"
    refuse_if_conflicted
    review="$(state_get "$n" change)"
    head="$(state_get "$n" head)"
    current_change >/dev/null
    if [ -n "$(jj_one "$review" 'change_id')" ]; then
        notes="$(jj diff --ignore-working-copy --from "$head" --to "$review" --name-only 2>/dev/null)" ||
            die "couldn't compare the review with #$n's head"
        if [ -n "$notes" ] && [ "$yes" != 1 ]; then
            echo "review-pr: the review of #$n has notes in:" >&2
            printf '  %s\n' "$notes" >&2
            die "they'd be lost; --pause keeps them, --stop --yes drops them"
        fi
    fi
    if [ "$(state_get "$n" status)" = active ]; then
        go_back "$n"
    fi
    if [ -n "$(jj_one "$review" 'change_id')" ]; then
        jj abandon "$review" >/dev/null 2>&1 || die "couldn't abandon the review change $review"
    fi
    rm -f "$state_dir/$n.state"
    echo "Stopped the review of #$n."
}

case "$action" in
    start) start_review ;;
    pause) pause_review ;;
    stop) stop_review ;;
    "") list_reviews ;;
esac
