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
        NODES_VERSION=v0.7.1 sh "$work/repo/scripts/tools-check.sh" --pins-only >"$work/out" 2>&1
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

# replace_in_ci <awk program> — rewrites ci.yml through awk.
replace_in_ci() {
    awk "$1" "$work/repo/.github/workflows/ci.yml" >"$work/ci.yml"
    mv "$work/ci.yml" "$work/repo/.github/workflows/ci.yml"
}

new_tree
replace_in_ci '{ sub(/version: .1\.72\.0./, "version: '"'"'1.71.0'"'"' # version: '"'"'1.72.0'"'"'"); print }'
expect_exit "a buf version only in a comment fails" 1

new_tree
replace_in_ci '/^          version: / { print "          # version: '"'"'1.72.0'"'"'"; next } { print }'
expect_exit "a buf step whose version is only a comment line fails" 1

new_tree
replace_in_ci '/^          just-version: / { print "          # just-version: '"'"'1.49.0'"'"'"; next } { print }'
expect_exit "a just pin only in a comment fails" 1

new_tree
replace_in_ci '/^          version: / { print "          version: '"'"'1.71.0'"'"'"; print "        env:"; print "          version: '"'"'1.72.0'"'"'"; next } { print }'
expect_exit "a buf pin under env: while with: disagrees fails" 1

new_tree
replace_in_ci '/^        with:/ && ++n == 1 { print "        env:"; next } { print }'
expect_exit "a just pin under env: instead of with: fails" 1

new_tree
replace_in_ci '/^          just-version: / { print "          just-version: \"1.49.0\" # pinned"; next } { print }'
expect_exit "a double-quoted just pin passes" 0

# expect_nodes_line <name> <nodes --version output> <pattern> — runs the local half
# with a `nodes` stub printing that output first on PATH; the exit code stays 0
# (local drift only warns) and the nodes line matches <pattern>. Every other
# tool the local half calls is stubbed too, so no real one runs (a real rustc or
# cargo through rustup could download the fixture's toolchain); their lines may
# warn: only the nodes line is read.
expect_nodes_line() {
    new_tree
    printf 'channel = "1.90.0"\n' >"$work/repo/rust-toolchain.toml"
    rm -rf "$work/bin"
    mkdir -p "$work/bin"
    for tool in typos cargo buf just rustc; do
        printf '#!/bin/sh\necho "%s 0.0.1"\n' "$tool" >"$work/bin/$tool"
        chmod +x "$work/bin/$tool"
    done
    printf '#!/bin/sh\necho "%s"\n' "$2" >"$work/bin/nodes"
    chmod +x "$work/bin/nodes"
    set +e
    PATH="$work/bin:$PATH" TYPOS_VERSION=1.51.1 CARGO_DENY_VERSION=0.20.2 BUF_VERSION=1.72.0 JUST_VERSION=1.49.0 \
        NODES_VERSION=v0.7.1 sh "$work/repo/scripts/tools-check.sh" >"$work/out" 2>&1
    got=$?
    set -e
    line="$(grep -F 'nodes' "$work/out" | grep -F 'tools-check:' | head -n 1 || true)"
    if [ "$got" -eq 0 ] && printf '%s\n' "$line" | grep -qE "$3"; then
        echo "ok   $1"
    else
        echo "FAIL $1: exit $got, nodes line '$line'"
        awk '{ print "  " $0 }' "$work/out"
        failures=$((failures + 1))
    fi
}

expect_nodes_line "a nodes older than the pin warns" "nodes 0.6.0" '^tools-check: warning: nodes is 0\.6\.0 here, CI uses 0\.7\.1 '
expect_nodes_line "a nodes at the pin matches" "nodes 0.7.1" '^tools-check: nodes 0\.7\.1 matches CI$'

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all tools-check tests passed"
