#!/bin/sh
# The body of CI's `changes` job: decides whether a pull request is docs-only,
# and whether a pull request or a merge-queue group changes Cargo.lock.
# Prints `code=true|false` and `lock=true|false` on stdout, its reasoning on
# stderr, and always exits 0.
#
# Env: GITHUB_EVENT_NAME, BASE_SHA and HEAD_SHA (the PR's or the merge
# group's base and head), and optionally CI_CHANGES_TMP (scratch directory;
# default: a fresh mktemp -d). Runs inside a checkout of HEAD: the PR's test
# merge commit, or the merge group's head commit.
#
# Fails closed: any other event, a missing SHA, a HEAD that isn't what the
# event names, or a failed diff or classification prints code=true lock=true.
# A pull request's merge commit must have exactly BASE_SHA and HEAD_SHA as
# parents. A merge group always prints code=true, since the queue is the last
# run before main; only its lock is classified, from BASE_SHA to HEAD, so the
# advisory rule matches the PR's. Classification uses the BASE commit's
# scripts/ci-paths.sh, never the change's own edit of it. CI also runs the
# base commit's copy of this file, for the same reason.

code=true
lock=true
own_tmp=""
trap 'if [ -n "$own_tmp" ]; then rm -rf "$own_tmp"; fi' EXIT

note() { echo "changes: $*" >&2; }

detect() {
    event="${GITHUB_EVENT_NAME:-}"
    case "$event" in
        pull_request | merge_group) ;;
        *)
            note "not a pull request or a merge group; running everything"
            return
            ;;
    esac
    for sha in "${BASE_SHA:-}" "${HEAD_SHA:-}"; do
        if ! printf '%s' "$sha" | grep -qE '^[0-9a-f]{40}$'; then
            note "base or head SHA missing; running everything"
            return
        fi
    done
    if [ "$event" = pull_request ]; then
        if ! parent1=$(git rev-parse --verify --quiet 'HEAD^1') ||
            ! parent2=$(git rev-parse --verify --quiet 'HEAD^2') ||
            [ "$parent1" != "$BASE_SHA" ] || [ "$parent2" != "$HEAD_SHA" ]; then
            note "the merge commit's parents are not the PR's base and head; running everything"
            return
        fi
        note "merge parents verified: HEAD^1 is the base, HEAD^2 is the PR head"
    else
        if ! head=$(git rev-parse --verify --quiet HEAD) || [ "$head" != "$HEAD_SHA" ] ||
            ! git merge-base --is-ancestor "$BASE_SHA" HEAD 2>/dev/null; then
            note "HEAD is not the merge group's head, or the base is not its ancestor; running everything"
            return
        fi
        note "merge group verified: HEAD is the group's head and descends from its base"
    fi
    tmp="${CI_CHANGES_TMP:-}"
    if [ -z "$tmp" ]; then
        if ! own_tmp=$(mktemp -d); then
            note "no scratch directory; running everything"
            return
        fi
        tmp="$own_tmp"
    fi
    if ! git show "$BASE_SHA:scripts/ci-paths.sh" >"$tmp/ci-paths.sh" 2>/dev/null; then
        note "the base has no scripts/ci-paths.sh; running everything"
        return
    fi
    # --no-renames: a moved file lists both its old and its new path.
    if ! git diff --no-renames --name-only "$BASE_SHA" HEAD >"$tmp/changed.txt"; then
        note "git diff failed; running everything"
        return
    fi
    if ! result=$(sh "$tmp/ci-paths.sh" <"$tmp/changed.txt"); then
        note "the classifier failed; running everything"
        return
    fi
    if [ "$event" = merge_group ]; then
        note "a merge group runs everything; only the advisory rule follows its paths"
    elif printf '%s\n' "$result" | grep -qx 'code=false'; then
        code=false
    fi
    if printf '%s\n' "$result" | grep -qx 'lock=false'; then
        lock=false
    fi
    note "changed paths:"
    cat "$tmp/changed.txt" >&2
}

detect
printf 'code=%s\nlock=%s\n' "$code" "$lock"
