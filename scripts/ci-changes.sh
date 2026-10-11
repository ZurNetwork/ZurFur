#!/bin/sh
# The body of CI's `changes` job: decides whether a pull request is docs-only.
# Prints `code=true|false` and `lock=true|false` on stdout, its reasoning on
# stderr, and always exits 0.
#
# Env: GITHUB_EVENT_NAME, BASE_SHA and HEAD_SHA (the PR's base and head), and
# optionally CI_CHANGES_TMP (scratch directory; default: a fresh mktemp -d).
# Runs inside a checkout of the PR's test merge commit (HEAD).
#
# Fails closed: unless this is a pull request whose merge commit's parents are
# exactly BASE_SHA and HEAD_SHA, whose diff and classification both succeed,
# it prints code=true lock=true. It classifies with the BASE commit's
# scripts/ci-paths.sh, never the PR's own edit of it. CI also runs the base
# commit's copy of this file, for the same reason.

code=true
lock=true
own_tmp=""
trap 'if [ -n "$own_tmp" ]; then rm -rf "$own_tmp"; fi' EXIT

note() { echo "changes: $*" >&2; }

detect() {
    if [ "${GITHUB_EVENT_NAME:-}" != pull_request ]; then
        note "not a pull request; running everything"
        return
    fi
    for sha in "${BASE_SHA:-}" "${HEAD_SHA:-}"; do
        if ! printf '%s' "$sha" | grep -qE '^[0-9a-f]{40}$'; then
            note "base or head SHA missing; running everything"
            return
        fi
    done
    if ! parent1=$(git rev-parse --verify --quiet 'HEAD^1') ||
        ! parent2=$(git rev-parse --verify --quiet 'HEAD^2') ||
        [ "$parent1" != "$BASE_SHA" ] || [ "$parent2" != "$HEAD_SHA" ]; then
        note "the merge commit's parents are not the PR's base and head; running everything"
        return
    fi
    note "merge parents verified: HEAD^1 is the base, HEAD^2 is the PR head"
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
    if printf '%s\n' "$result" | grep -qx 'code=false'; then
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
