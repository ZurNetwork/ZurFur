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

# new_tree — a repo whose ci.yml pins everything correctly and keeps every CI
# safety rule: read-only top-level permissions, checkouts without stored
# credentials, and the guarded install.
new_tree() {
    rm -rf "$work/repo"
    mkdir -p "$work/repo/scripts" "$work/repo/.github/workflows"
    cp "$script_dir/tools-check.sh" "$work/repo/scripts/"
    cat >"$work/repo/.github/workflows/ci.yml" <<EOF
on:
  pull_request:

permissions:
  contents: read

jobs:
  a:
    steps:
      - uses: actions/checkout@$sha # v7.0.1
        with:
          persist-credentials: false
      - uses: crate-ci/typos@$sha # v1.51.1
      - uses: EmbarkStudios/cargo-deny-action@$sha # v2.1.1 (cargo-deny 0.20.2)
      - uses: extractions/setup-just@$sha # v4.0.0
        with:
          just-version: '1.49.0'
      - uses: bufbuild/buf-action@$sha # v1.6.0
        with:
          version: '1.72.0'
  web:
    defaults:
      run:
        working-directory: frontend/web
    steps:
      - uses: actions/checkout@$sha # v7.0.1
        with:
          fetch-depth: 0
          persist-credentials: false
      - uses: actions/setup-node@$sha # v7.1.0
        with:
          cache: yarn
          cache-dependency-path: frontend/yarn.lock
      # The guarded install.
      - run: node frontend/install-check.mjs
        working-directory: .
      - run: yarn --cwd frontend install --frozen-lockfile --ignore-scripts
        working-directory: .
      - name: guard the installed packages
        run: node frontend/install-check.mjs --installed
        working-directory: .
      - run: yarn playwright install --with-deps chromium
      - run: |
          yarn run check
          yarn --cwd frontend/web run build
EOF
}

# expect_exit <name> <expected exit> [<fragment>] — and, given, the output holds <fragment>.
expect_exit() {
    set +e
    TYPOS_VERSION=1.51.1 CARGO_DENY_VERSION=0.20.2 BUF_VERSION=1.72.0 JUST_VERSION=1.49.0 \
        NODES_VERSION=v0.7.1 sh "$work/repo/scripts/tools-check.sh" --pins-only >"$work/out" 2>&1
    got=$?
    set -e
    if [ "$got" -ne "$2" ]; then
        echo "FAIL $1: exit $got (want $2)"
        awk '{ print "  " $0 }' "$work/out"
        failures=$((failures + 1))
    elif [ -n "${3:-}" ] && ! grep -qF -- "$3" "$work/out"; then
        echo "FAIL $1: output lacks '$3'"
        awk '{ print "  " $0 }' "$work/out"
        failures=$((failures + 1))
    else
        echo "ok   $1"
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
replace_in_ci '/setup-just@/ { s = 1 } s && /^        with:/ { print "        env:"; s = 0; next } { print }'
expect_exit "a just pin under env: instead of with: fails" 1 "set just-version"

new_tree
replace_in_ci '/^          just-version: / { print "          just-version: \"1.49.0\" # pinned"; next } { print }'
expect_exit "a double-quoted just pin passes" 0

# The CI safety rules: each break alone fails.

new_tree
replace_in_ci '/persist-credentials: false/ && ++n == 2 { next } { print }'
expect_exit "a checkout without persist-credentials: false fails" 1 "set persist-credentials: 'false'"

new_tree
replace_in_ci '/persist-credentials: false/ && ++n == 1 { print "          persist-credentials: true"; next } { print }'
expect_exit "a checkout with persist-credentials: true fails" 1 "set persist-credentials: 'false'"

new_tree
replace_in_ci '{ print } /^  a:$/ { print "    permissions:"; print "      contents: read" }'
expect_exit "a job-level permissions: fails" 1 "a permissions: below the top level"

new_tree
replace_in_ci '/^permissions:$/ { skip = 1; next } skip && /^  / { next } { skip = 0; print }'
expect_exit "a workflow without top-level permissions fails" 1 "no top-level permissions"

new_tree
printf 'name: other\njobs:\n  b:\n    steps:\n      - run: echo hi\n' >"$work/repo/.github/workflows/other.yml"
expect_exit "a second workflow without top-level permissions fails" 1 "other.yml:1: no top-level permissions"

new_tree
replace_in_ci '/^  contents: read$/ { print "  contents: write"; next } { print }'
expect_exit "top-level contents: write fails" 1 "does not hold contents: read"

new_tree
replace_in_ci '{ print } /^  contents: read$/ { print "  packages: write" }'
expect_exit "a top-level write permission beside contents: read fails" 1 "other than read or none"

new_tree
replace_in_ci '/^permissions:$/ { print "permissions: write-all"; skip = 1; next } skip && /^  / { next } { skip = 0; print }'
expect_exit "an inline top-level permissions fails" 1 "does not hold contents: read"

new_tree
replace_in_ci '{ print } /^  pull_request:$/ { print "  pull_request_target:" }'
expect_exit "pull_request_target fails" 1 "pull_request_target"

new_tree
printf '      - run: echo "${{ secrets.TOKEN }}"\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a secrets. expression fails" 1 "a secrets expression"

new_tree
printf '      - run: |\n          # ${{ secrets.TOKEN }}\n          true\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a secrets. expression in a run script comment fails" 1 "a secrets expression"

new_tree
printf '      - env:\n          T: ${{ secrets["TOKEN"] }}\n        run: true\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a secrets[ expression fails" 1 "a secrets expression"

new_tree
replace_in_ci '{ sub(/--frozen-lockfile --ignore-scripts/, "--frozen-lockfile"); print }'
expect_exit "a yarn --cwd frontend install without --ignore-scripts fails" 1 "without --ignore-scripts"

new_tree
replace_in_ci '{ sub(/--frozen-lockfile --ignore-scripts/, "--ignore-scripts"); print }'
expect_exit "a yarn --cwd frontend install without --frozen-lockfile fails" 1 "without --frozen-lockfile"

new_tree
replace_in_ci '/run: node frontend\/install-check.mjs$/ { getline; next } { print }'
expect_exit "an install without the guard before it fails" 1 "as the step before it"

new_tree
replace_in_ci '/install-check.mjs --installed/ { getline; next } /name: guard the installed packages/ { next } { print }'
expect_exit "an install without the guard after it fails" 1 "--installed\` as the step after it"

new_tree
replace_in_ci '/run: node frontend\/install-check.mjs$/ { print "      - run: node frontend/install-check.mjs || true"; next } { print }'
expect_exit "a guard that can't fail is not the guard" 1 "as the step before it"

new_tree
replace_in_ci '{ print } /run: node frontend\/install-check.mjs$/ { print "        continue-on-error: true" }'
expect_exit "a guard step with continue-on-error fails" 1 "the install guard step sets more than"

new_tree
printf '      - run: yarn\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a bare yarn install fails" 1 "without --frozen-lockfile"

new_tree
printf '      - run: cd frontend && yarn add left-pad --frozen-lockfile --ignore-scripts\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a yarn add in a compound command fails" 1 "shares its step with other commands"

new_tree
printf '      - run: yarn --network-timeout 100000 --cwd frontend install\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "flags with values before install still count as an install" 1 "without --ignore-scripts"

new_tree
printf '      - run: |\n          yarn --cwd frontend \\\n            install\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "an install split over a continued line fails" 1 "without --ignore-scripts"

new_tree
printf '      - run: >-\n          yarn --cwd frontend\n          install\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "an install split over a folded scalar fails" 1 "without --ignore-scripts"

new_tree
printf '      - {name: x, run: yarn install}\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a flow-style step it can't read fails" 1 "a step tools-check cannot read"

new_tree
printf '    container:\n      options: [{run: yarn install}]\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a run: outside any step fails" 1 "a run: that is not a step command"

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
