#!/bin/sh
# The tests dispatcher behind the gate's tests template: one test name in, the
# run that fits it out. A *.spec.ts name (relative to frontend/ui) runs under
# Vitest, a *.stories.svelte name runs in the two Storybook projects, and any
# other name is a cargo test filter over the workspace. Exits with the status
# of the run it started; no name, or more than one, exits 2 and starts nothing.
# Works from any working directory. Run `scripts/zaf-tests.sh <name>`.
set -eu

if [ "$#" -ne 1 ]; then
    echo "usage: zaf-tests.sh <name>" >&2
    exit 2
fi
name="$1"

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"

case "$name" in
    *.spec.ts)
        exec yarn --cwd frontend/ui run test "$name"
        ;;
    *.stories.svelte)
        exec yarn --cwd frontend/ui run test --project stories --project stories-light-still "$name"
        ;;
    *)
        exec cargo test --workspace "$name"
        ;;
esac
