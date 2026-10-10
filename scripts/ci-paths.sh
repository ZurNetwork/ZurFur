#!/bin/sh
# Classifies a change's paths for CI's `changes` job and for `just gate-light`.
# Reads one repo-relative path per line on stdin.
#
#   ci-paths.sh           prints `code=true|false` and `lock=true|false`
#   ci-paths.sh --paths   prints `docs <path>` or `code <path>` per path
#
# code=true unless there is at least one path and every path is docs-only, so
# an unknown path or a quoted name git could not print plainly counts as code.
# lock=true when a path is a Cargo.lock, or when there are no paths at all.
# Both fail closed: CI treats any output but an explicit `false` as true.
#
# CI runs the BASE branch's copy of this file, so a PR that edits the list
# cannot use its own edit to skip its own jobs. Keep the list in step with the
# reasons in the CI/CD study's Q5 path table.
set -eu

# docs_only <path> succeeds when no Rust, web or contract job reads <path>.
# The always-on jobs (typos, nodes, hooks) still check every one of these.
docs_only() {
    case "$1" in
        # Markdown anywhere: no Rust, web or contract code reads one.
        *.md) return 0 ;;
        # Read only by the `nodes` job.
        NODE.json | */NODE.json | NODE.toml) return 0 ;;
        # design-index.md (nodes job); documents; boardroom runs; agent settings.
        docs/* | .understand/* | reports/* | forks/* | .claude/*) return 0 ;;
        # The board tool's config; parsed by the always-on `hooks` job.
        .agent-board.toml) return 0 ;;
        # Used by the always-on `nodes` and `hooks` jobs and locally.
        scripts/* | Justfile) return 0 ;;
    esac
    return 1
}

is_lockfile() {
    case "$1" in
        Cargo.lock | */Cargo.lock) return 0 ;;
    esac
    return 1
}

mode="${1:-summary}"
case "$mode" in
    summary | --paths) ;;
    *)
        echo "usage: ci-paths.sh [--paths] < changed-paths" >&2
        exit 2
        ;;
esac

seen=0
code=false
lock=false
while IFS= read -r path || [ -n "$path" ]; do
    [ -n "$path" ] || continue
    seen=1
    if docs_only "$path"; then
        kind=docs
    else
        kind=code
        code=true
    fi
    if is_lockfile "$path"; then
        lock=true
    fi
    if [ "$mode" = "--paths" ]; then
        printf '%s %s\n' "$kind" "$path"
    fi
done

if [ "$mode" = summary ]; then
    if [ "$seen" = 0 ]; then
        code=true
        lock=true
    fi
    printf 'code=%s\nlock=%s\n' "$code" "$lock"
fi
