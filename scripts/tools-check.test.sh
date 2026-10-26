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

# install_line <command> — frontend/install-check.mjs's INSTALL_COMMAND line.
install_line() {
    printf "export const INSTALL_COMMAND = '%s';\n" "$1"
}

# new_tree — a repo whose ci.yml pins everything correctly and keeps every CI
# safety rule: read-only top-level permissions, checkouts without stored
# credentials, and the guarded install, exactly the install guard's command.
new_tree() {
    rm -rf "$work/repo"
    mkdir -p "$work/repo/scripts" "$work/repo/.github/workflows" "$work/repo/frontend"
    cp "$script_dir/tools-check.sh" "$work/repo/scripts/"
    install_line 'yarn --cwd frontend install --frozen-lockfile --ignore-scripts' >"$work/repo/frontend/install-check.mjs"
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

# The guarded install is exactly frontend/install-check.mjs's INSTALL_COMMAND.
exact="a yarn install that is not exactly \`yarn --cwd frontend install --frozen-lockfile --ignore-scripts\`"

new_tree
replace_in_ci '/run: yarn --cwd frontend install/ { print "      - run: yarn --use-yarnrc frontend/ui/x.yarnrc --cwd frontend install --frozen-lockfile --ignore-scripts --registry https://x.invalid/"; next } { print }'
expect_exit "a guarded install with --use-yarnrc and --registry fails" 1 "$exact"
expect_exit "a guarded install with --use-yarnrc names the flag" 1 "a yarn --use-yarnrc"

new_tree
replace_in_ci '{ sub(/--ignore-scripts$/, "--ignore-scripts --registry https://x.invalid/"); print }'
expect_exit "a guarded install with an extra flag fails" 1 "$exact"

new_tree
replace_in_ci '{ sub(/--frozen-lockfile --ignore-scripts$/, "--ignore-scripts --frozen-lockfile"); print }'
expect_exit "a guarded install with its flags reordered fails" 1 "$exact"

new_tree
replace_in_ci '/run: yarn --cwd frontend install/ { print "      - run: yarn install --frozen-lockfile --ignore-scripts"; print "        working-directory: frontend"; getline; next } { print }'
expect_exit "a guarded install run from frontend/ fails" 1 "$exact"

new_tree
install_line 'yarn --cwd frontend install --frozen-lockfile --ignore-scripts --offline' >"$work/repo/frontend/install-check.mjs"
expect_exit "an install that differs from install-check.mjs's INSTALL_COMMAND fails" 1 "is not exactly \`yarn --cwd frontend install --frozen-lockfile --ignore-scripts --offline\`"

new_tree
printf 'export const INSTALL = 1;\n' >"$work/repo/frontend/install-check.mjs"
expect_exit "an install-check.mjs without INSTALL_COMMAND fails" 1 "no INSTALL_COMMAND"

new_tree
install_line 'yarn --cwd frontend install --frozen-lockfile --ignore-scripts' >>"$work/repo/frontend/install-check.mjs"
expect_exit "an install-check.mjs with two INSTALL_COMMAND lines fails" 1 "no INSTALL_COMMAND"

# Every other installer, and every yarn command that fetches and runs a
# package or changes which yarn runs, fails in any run:.

# expect_step_fails <name> <run value> <fragment> — a step running <run value> fails with <fragment>.
expect_step_fails() {
    new_tree
    printf '      - run: %s\n' "$2" >>"$work/repo/.github/workflows/ci.yml"
    expect_exit "$1" 1 "$3"
}

expect_step_fails "npx fails" 'npx --yes cowsay hi' "a package manager other than yarn: npx"
expect_step_fails "yarn create fails" 'yarn create vite x' "a refused yarn subcommand: create"
expect_step_fails "npm install fails" 'npm install left-pad' "a package manager other than yarn: npm"
expect_step_fails "pnpm fails" 'pnpm i' "a package manager other than yarn: pnpm"
expect_step_fails "pnpx fails" 'pnpx y' "a package manager other than yarn: pnpx"
expect_step_fails "bun fails" 'bun x y' "a package manager other than yarn: bun"
expect_step_fails "bunx fails" 'bunx y' "a package manager other than yarn: bunx"
expect_step_fails "corepack fails" 'corepack enable' "a package manager other than yarn: corepack"
expect_step_fails "yarn policies fails" 'yarn policies set-version 1.22.22' "a refused yarn subcommand: policies"
expect_step_fails "an npm by path fails" '/usr/bin/npm ci' "a package manager other than yarn: npm"
expect_step_fails "a quoted npm fails" '"npm" ci' "a package manager other than yarn: npm"
expect_step_fails "an npx after an env assignment fails" 'FOO=1 npx x' "a package manager other than yarn: npx"
expect_step_fails "an npm behind a wrapper fails" 'env -i nice -n 19 npm ci' "a package manager other than yarn: npm"
expect_step_fails "an npm in sh -c fails" "sh -c 'npm ci'" "a package manager other than yarn: npm"
expect_step_fails "an npm after a quoted # fails" 'echo "#"; npm ci' "a package manager other than yarn: npm"
expect_step_fails "an npm with a redirect fails" 'npm>/dev/null ci' "a package manager other than yarn: npm"
expect_step_fails "yarn set version fails" 'yarn set version berry' "a refused yarn subcommand: set"
expect_step_fails "yarn init fails" 'yarn init -2' "a refused yarn subcommand: init"
expect_step_fails "yarn config fails" 'yarn config set yarn-path x.js' "a refused yarn subcommand: config"
expect_step_fails "yarn workspace fails" 'yarn workspace web add left-pad' "a refused yarn subcommand: workspace"
expect_step_fails "yarn create after a flag bound to a value fails" 'yarn --emoji true create x' "a refused yarn subcommand: create"
expect_step_fails "yarn create after flags fails" 'yarn --cwd frontend --silent create x' "a refused yarn subcommand: create"
expect_step_fails "yarn unplug counts as an install" 'yarn --cwd frontend unplug x' "$exact"
expect_step_fails "a --use-yarnrc on any yarn command fails" 'yarn run check --use-yarnrc x.yarnrc' "a yarn --use-yarnrc"

new_tree
printf '      - run: |\n          echo hi\n          npm ci\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "an npm on a later line of a run block fails" 1 "a package manager other than yarn: npm"

new_tree
printf '      - run: >-\n          echo hi \\\n          && corepack enable\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a corepack in a folded, continued run fails" 1 "a package manager other than yarn: corepack"

new_tree
printf '      - run: echo hi\n        shell: npx x {0}\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "an npx as a step's shell fails" 1 "a package manager other than yarn: npx"

new_tree
replace_in_ci '{ print } /^        working-directory: frontend\/web$/ { print "        shell: bunx sh {0}" }'
expect_exit "a bunx as the defaults shell fails" 1 "a package manager other than yarn: bunx"

new_tree
printf '      - {name: x, run: yarn install}\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a flow-style step it can't read fails" 1 "a step tools-check cannot read"

new_tree
printf '    container:\n      options: [{run: yarn install}]\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a run: outside any step fails" 1 "a run: that is not a step command"

# Every uses: is a step's, naming exactly one action on the allow list.
not_allowed="an action not on the allow list:"

# expect_use <name> <expected exit> <uses value> [<fragment>] — a step using <uses value>.
expect_use() {
    new_tree
    printf '      - uses: %s\n' "$3" >>"$work/repo/.github/workflows/ci.yml"
    expect_exit "$1" "$2" "${4:-}"
}

expect_use "a step using rust-cache passes" 0 "Swatinem/rust-cache@$sha # v2.9.2"
expect_use "a SHA-pinned action off the allow list fails" 1 "bahmutov/npm-install@$sha # v1.10.0" "$not_allowed bahmutov/npm-install"
expect_use "a local action fails" 1 "./.github/actions/x" "$not_allowed ./.github/actions/x"
expect_use "a docker image fails" 1 "docker://alpine@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef" "$not_allowed docker://alpine"
expect_use "a case-changed action name fails" 1 "Actions/checkout@$sha # v7.0.1" "$not_allowed Actions/checkout"
expect_use "a sub-path of an allowed action fails" 1 "actions/checkout/sub@$sha # v7.0.1" "$not_allowed actions/checkout/sub"

new_tree
at="$(($(wc -l <"$work/repo/.github/workflows/ci.yml") + 2))"
printf '  b:\n    uses: other/repo/.github/workflows/x.yml@%s # v1.0.0\n' "$sha" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a job-level uses: fails" 1 "ci.yml:$at: a uses: that is not a step key"
expect_exit "a job-level uses: names its workflow" 1 "ci.yml:$at: $not_allowed other/repo/.github/workflows/x.yml"

new_tree
printf '      - { uses: actions/checkout@%s }\n' "$sha" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a flow-style uses: step fails" 1 "a step tools-check cannot read"

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
