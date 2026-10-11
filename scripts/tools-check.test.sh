#!/bin/sh
# Tests for scripts/tools-check.sh --pins-only against scratch workflow trees.
# Run directly, or via `just scripts-test`.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

failures=0
sha=0123456789abcdef0123456789abcdef01234567

# new_tree — a repo whose ci.yml pins everything correctly.
new_tree() {
    rm -rf "$work/repo"
    mkdir -p "$work/repo/scripts" "$work/repo/.github/workflows"
    cp "$script_dir/tools-check.sh" "$work/repo/scripts/"
    cat >"$work/repo/.github/workflows/ci.yml" <<EOF
jobs:
  a:
    steps:
      - uses: actions/checkout@$sha # v7.0.1
      - uses: crate-ci/typos@$sha # v1.51.1
      - uses: EmbarkStudios/cargo-deny-action@$sha # v2.1.1 (cargo-deny 0.20.2)
      - uses: extractions/setup-just@$sha # v4.0.0
        with:
          just-version: '1.49.0'
      - uses: bufbuild/buf-action@$sha # v1.6.0
        with:
          version: '1.72.0'
EOF
}

# expect_exit <name> <expected exit>
expect_exit() {
    set +e
    TYPOS_VERSION=1.51.1 CARGO_DENY_VERSION=0.20.2 BUF_VERSION=1.72.0 JUST_VERSION=1.49.0 \
        sh "$work/repo/scripts/tools-check.sh" --pins-only >"$work/out" 2>&1
    got=$?
    set -e
    if [ "$got" -eq "$2" ]; then
        echo "ok   $1"
    else
        echo "FAIL $1: exit $got (want $2)"
        awk '{ print "  " $0 }' "$work/out"
        failures=$((failures + 1))
    fi
}

new_tree
expect_exit "every pin in place passes" 0

new_tree
printf '      - uses: owner/action@main # @%s # v1\n' "$sha" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a mutable ref with a SHA in its comment fails" 1

new_tree
printf 'jobs:\n  b:\n    steps:\n      - uses: owner/action@v1\n' >"$work/repo/.github/workflows/other.yaml"
expect_exit "an unpinned action in a .yaml workflow fails" 1

new_tree
printf '      - uses: extractions/setup-just@%s # v4.0.0\n' "$sha" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a setup-just step without the just pin fails" 1

new_tree
awk '{ sub(/# v1\.51\.1/, "# v1.50.0"); print }' "$work/repo/.github/workflows/ci.yml" >"$work/ci.yml"
mv "$work/ci.yml" "$work/repo/.github/workflows/ci.yml"
expect_exit "a typos pin that differs from the Justfile fails" 1

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all tools-check tests passed"
