#!/bin/sh
# Tests for scripts/tools-check.sh --pins-only against scratch workflow trees.
# Run directly, or via `just scripts-test`.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

failures=0
# A well-formed SHA that is no action's reviewed commit.
sha=0123456789abcdef0123456789abcdef01234567

# reviewed <action> — "<action>@<SHA>" from tools-check.sh's allowed_actions,
# so the fixture uses the one table there rather than a second copy.
reviewed() {
    ref="$(awk -v name="$1" '
        /^allowed_actions="/ { on = 1; next }
        on && /^"/ { exit }
        on && index($0, name "@") == 1 { print; exit }
    ' "$script_dir/tools-check.sh")"
    if [ -z "$ref" ]; then
        echo "FAIL setup: no $1 line in tools-check.sh's allowed_actions" >&2
        exit 1
    fi
    printf '%s\n' "$ref"
}
checkout="$(reviewed actions/checkout)"
setup_node="$(reviewed actions/setup-node)"
rust_cache="$(reviewed Swatinem/rust-cache)"
buf_action="$(reviewed bufbuild/buf-action)"
cargo_deny="$(reviewed EmbarkStudios/cargo-deny-action)"
typos="$(reviewed crate-ci/typos)"
setup_just="$(reviewed extractions/setup-just)"
upload_artifact="$(reviewed actions/upload-artifact)"

# install_line <command> — frontend/install-check.mjs's INSTALL_COMMAND line.
install_line() {
    printf "export const INSTALL_COMMAND = '%s';\n" "$1"
}

# new_tree — a repo whose ci.yml pins everything correctly and keeps every CI
# safety rule: read-only top-level permissions, checkouts without stored
# credentials, the guarded install, exactly the install guard's command, and
# the one failure upload: the screenshot run's PNG files, kept 7 days.
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
      - uses: $checkout # v7.0.1
        with:
          persist-credentials: false
      - uses: $typos # v1.51.1
      - uses: $cargo_deny # v2.1.1 (cargo-deny 0.20.2)
      - uses: $setup_just # v4.0.0
        with:
          just-version: '1.49.0'
      - uses: $buf_action # v1.6.0
        with:
          version: '1.72.0'
  web:
    defaults:
      run:
        working-directory: frontend/web
    steps:
      - uses: $checkout # v7.0.1
        with:
          fetch-depth: 0
          persist-credentials: false
      - uses: $setup_node # v7.1.0
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
      - uses: $upload_artifact # v7.0.2
        if: failure()
        with:
          name: gallery-shots-failures
          path: frontend/ui/shots/output/**/*.png
          retention-days: 7
          if-no-files-found: ignore
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
printf '      - uses: %s # v4.0.0\n' "$setup_just" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a setup-just step without the just pin fails" 1 "set just-version"

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

# The one upload: the screenshot run's PNG files, kept 7 days, and nothing wider.
new_tree
replace_in_ci '/retention-days: 7/ { print "          retention-days: 90"; next } { print }'
expect_exit "an upload kept longer than 7 days fails" 1 "set retention-days: '7'"

new_tree
replace_in_ci '/retention-days: 7/ { next } { print }'
expect_exit "an upload without retention-days fails" 1 "set retention-days: '7'"

new_tree
replace_in_ci '/path: frontend\/ui\/shots\/output/ { print "          path: frontend/ui/"; next } { print }'
expect_exit "an upload of a wider path fails" 1 "set path: 'frontend/ui/shots/output/**/*.png'"

new_tree
replace_in_ci '/path: frontend\/ui\/shots\/output/ { print "          path: |"; print "            frontend/ui/shots/output/**/*.png"; print "            frontend/ui/storybook-static"; next } { print }'
expect_exit "an upload of a path list fails" 1 "set path: 'frontend/ui/shots/output/**/*.png'"

new_tree
printf '      - uses: %s # v7.0.2\n        with:\n          name: x\n          path: frontend/ui/shots/output/**/*.png\n' "$upload_artifact" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a second upload without retention-days fails" 1 "2 actions/upload-artifact step(s) but 1 set retention-days"

# No workflow asks for a container itself: no container: or services: key,
# and no docker or podman in a run: or shell:.
new_tree
at="$(($(wc -l <"$work/repo/.github/workflows/ci.yml") + 2))"
printf '  b:\n    container: node:24\n    steps:\n      - run: true\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a job's container: fails" 1 "ci.yml:$at: a container: key"

new_tree
replace_in_ci '{ print } /^  web:$/ { print "    container:"; print "      image: node:24" }'
expect_exit "a block container: under a job fails" 1 "a container: key"

new_tree
at="$(($(wc -l <"$work/repo/.github/workflows/ci.yml") + 2))"
printf '  b:\n    services:\n      db:\n        image: postgres:17\n    steps:\n      - run: true\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a job's services: fails" 1 "ci.yml:$at: a services: key"

new_tree
printf '  b: {runs-on: x, services: {db: {image: postgres}}, steps: [{run: true}]}\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a flow-style services: fails" 1 "a services: key"

new_tree
printf '      - name: x\n        container: alpine\n        run: true\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a step's container: fails" 1 "a container: key"

new_tree
printf '      - name: x\n        container : alpine\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a container : key with a space before its colon fails" 1 "a container: key"

docker_run="docker in a run: or shell:"
podman_run="podman in a run: or shell:"
expect_step_fails "a docker in a run: fails" 'docker ps' "$docker_run"
expect_step_fails "a podman in a run: fails" 'podman ps' "$podman_run"
expect_step_fails "a docker by path fails" '/usr/bin/docker ps' "$docker_run"
expect_step_fails "an upper-case DOCKER fails" 'DOCKER ps' "$docker_run"
expect_step_fails "a Podman in mixed case fails" 'PoDmAn run x' "$podman_run"
expect_step_fails "a quoted docker fails" '"docker" ps' "$docker_run"
expect_step_fails "a docker split by quotes fails" "do''cker ps" "$docker_run"
expect_step_fails "a docker behind a wrapper fails" 'sudo -E docker run x' "$docker_run"
expect_step_fails "a docker in sh -c fails" "sh -c 'docker ps'" "$docker_run"
expect_step_fails "a docker in a command substitution fails" 'echo "$(docker ps)"' "$docker_run"
expect_step_fails "a docker-compose fails" 'docker-compose up' "$docker_run"

new_tree
printf '      - run: |\n          echo hi\n          podman ps\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a podman on a later line of a run block fails" 1 "$podman_run"

new_tree
printf '      - run: |\n          dock\\\n          er ps\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a docker split over a continued line fails" 1 "$docker_run"

new_tree
printf '      - run: ps\n        shell: podman exec x sh {0}\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a podman as a step's shell fails" 1 "$podman_run"

new_tree
printf '      - run: |\n          # docker and podman stay out of this step\n          echo $DOCKER_HOST dockerfile\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "docker in a run's comment or inside a longer word passes" 0

new_tree
replace_in_ci '{ print } /^  web:$/ { print "    name: no container, no docker" }'
expect_exit "docker in a plain value outside a run: passes" 0

# Every uses: is a step's, naming exactly one action on the allow list.
not_allowed="an action not on the allow list:"

# expect_use <name> <expected exit> <uses value> [<fragment>] — a step using <uses value>.
expect_use() {
    new_tree
    printf '      - uses: %s\n' "$3" >>"$work/repo/.github/workflows/ci.yml"
    expect_exit "$1" "$2" "${4:-}"
}

expect_use "a step using rust-cache passes" 0 "$rust_cache # v2.9.2"
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
printf '      - { uses: %s }\n' "$checkout" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a flow-style uses: step fails" 1 "a step tools-check cannot read"

# An allowed action is used only at the one SHA allowed_actions lists for it:
# GitHub fetches any SHA under its name, a fork's included.
expect_use "setup-node at its reviewed SHA passes" 0 "$setup_node # v7.1.0"
expect_use "setup-node at another SHA fails" 1 "actions/setup-node@$sha # v7.1.0" \
    "an action not at its reviewed commit: actions/setup-node@$sha"
expect_use "checkout at another SHA fails" 1 "actions/checkout@$sha # v7.0.1" \
    "an action not at its reviewed commit: actions/checkout@$sha"
expect_use "upload-artifact at another SHA fails" 1 "actions/upload-artifact@$sha # v7.0.2" \
    "an action not at its reviewed commit: actions/upload-artifact@$sha"
expect_use "a reviewed SHA with more after its @ fails" 1 "$checkout@$sha # v7.0.1" \
    "an action not at its reviewed commit: $checkout@$sha"

# No mapping key is quoted and no key is explicit (`?`): YAML reads each as the
# bare key, so a job-level `uses:` spelled that way would call another repo's
# reusable workflow unseen.
quoted_key="a quoted mapping key, a shape tools-check cannot read"
workflow="other/repo/.github/workflows/x.yml@$sha"

# expect_job <name> <job lines> <fragment> — appends job b, written as <job
# lines> (printf's format, %s is the workflow it calls), and expects exit 1.
expect_job() {
    new_tree
    at="$(($(wc -l <"$work/repo/.github/workflows/ci.yml") + 2))"
    # shellcheck disable=SC2059 # the job lines are the format on purpose
    printf "  b:\n$2" "$workflow" >>"$work/repo/.github/workflows/ci.yml"
    expect_exit "$1" 1 "ci.yml:$at: $3"
}

expect_job "a single-quoted job-level 'uses': fails" "    'uses': %s\n" "$quoted_key"
expect_job "a double-quoted job-level \"uses\": fails" '    "uses": %s\n' "$quoted_key"
expect_job "a double-quoted, escaped job-level \"u\\x73es\": fails" '    "u\\x73es": %s\n' "$quoted_key"
expect_job "a quoted job-level key with a space before its colon fails" "    'uses' : %s\n" "$quoted_key"
expect_job "an explicit job-level ? uses fails" '    ? uses\n    : %s\n' "an explicit key (?), a shape tools-check cannot read"

new_tree
printf '  b: {"uses": %s}\n' "$workflow" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a flow-style job {\"uses\": ...} fails" 1 "$quoted_key"

new_tree
printf '  b: {name: don'"'"'t, "uses": %s}\n' "$workflow" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a flow-style quoted key after a plain value with an apostrophe fails" 1 "$quoted_key"

new_tree
printf '  b: {name: x, ? uses : %s}\n' "$workflow" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a flow-style explicit key fails" 1 "an explicit key (?)"

new_tree
printf '  b: {name: "x\n    ", "uses": %s}\n' "$workflow" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a quoted scalar over two lines that hides a quoted key fails" 1 "a quoted scalar that runs past its line"

new_tree
replace_in_ci '{ print } /^  web:$/ { print "    env:"; print "      '"'"'FOO'"'"': bar" }'
expect_exit "a quoted key under env: fails" 1 "$quoted_key"

# A tag, an anchor or an alias where a key starts can dress or stand for a
# `uses` key this reader never sees.
expect_job "a tag before a quoted job-level key fails" '    !!str "uses": %s\n' \
    "a tag (!) at a key position, a shape tools-check cannot read"
expect_job "an anchor before a quoted job-level key fails" '    &a "uses": %s\n' \
    "an anchor (&) at a key position, a shape tools-check cannot read"

new_tree
at="$(($(wc -l <"$work/repo/.github/workflows/ci.yml") + 3))"
printf '  b:\n    name: &k uses\n    *k : %s\n' "$workflow" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "an alias as a job-level key fails" 1 "ci.yml:$at: an alias (*) at a key position"

new_tree
printf '  b: {name: &k uses, *k : %s}\n' "$workflow" >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a flow-style alias key fails" 1 "an alias (*) at a key position"

# A line break awk does not split on hides the line after it, which YAML reads.

# expect_break <name> <bytes> <fragment> — job b holds <bytes> (printf's
# format) then a quoted `uses` key on what awk reads as the same line.
expect_break() {
    new_tree
    at="$(($(wc -l <"$work/repo/.github/workflows/ci.yml") + 1))"
    # shellcheck disable=SC2059 # the bytes are the format on purpose
    printf "  b:$2    \"uses\": %s\n" "$workflow" >>"$work/repo/.github/workflows/ci.yml"
    expect_exit "$1" 1 "ci.yml:$at: $3, a line break tools-check cannot read"
}

expect_break "a lone CR before a quoted job-level key fails" '\r' "a lone CR (not a CRLF line end)"
expect_break "a NEL before a quoted job-level key fails" '\302\205' "a NEL (U+0085)"
expect_break "an LS before a quoted job-level key fails" '\342\200\250' "an LS (U+2028)"
expect_break "a PS before a quoted job-level key fails" '\342\200\251' "a PS (U+2029)"

new_tree
printf '      - run: echo hi\r  npm ci\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a lone CR in a run script fails" 1 "a lone CR (not a CRLF line end)"

new_tree
printf '      - run: echo hi\r\r\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a CR before a CRLF line end fails" 1 "a lone CR (not a CRLF line end)"

new_tree
replace_in_ci '{ printf "%s\r\n", $0 }'
expect_exit "a workflow with CRLF line ends passes" 0

# A tag, an anchor or an alias on a value is not a key.
new_tree
replace_in_ci '{ print } /^  web:$/ { print "    env:"; print "      A: &a value"; print "      B: *a"; print "      C: !!str value" }'
expect_exit "a tag, an anchor and an alias on values pass" 0

new_tree
replace_in_ci '{ print } /^  web:$/ { print "    if: ${{ !cancelled() && needs.a.outputs.code != '"'"'false'"'"' }}" }'
expect_exit "a ! inside a \${{ }} expression passes" 0

new_tree
printf '      - name: x\n        "run": yarn install\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "a quoted run key in a step fails" 1 "$quoted_key"

# A quoted value is not a key, and a run: script is not YAML keys.
new_tree
replace_in_ci '{ print } /^  web:$/ { print "    if: ${{ github.event_name == '"'"'push'"'"' && contains(github.ref, '"'"'main'"'"') }}" }'
expect_exit "a quoted string inside an if: expression passes" 0

new_tree
replace_in_ci '{ print } /^  pull_request:$/ { print "    branches:"; print "      - '"'"'main'"'"'"; print "      - \"release/*\""; print "    paths: ['"'"'a:b'"'"', \"c\"]" }'
expect_exit "quoted list items and flow-list values pass" 0

new_tree
replace_in_ci '/cache-dependency-path:/ { print "          cache-dependency-path: \"frontend/yarn.lock\" # a: \"b\": c"; next } { print }'
expect_exit "a double-quoted value with a quoted key in its comment passes" 0

new_tree
replace_in_ci '/^          just-version: / { print "          just-version: '"'"'1.49.0'"'"'"; next } { print }'
expect_exit "a single-quoted just pin passes" 0

new_tree
printf '      - name: it'"'"'s a "x" step\n        run: |\n          echo '"'"'{"uses": 1}'"'"'\n          "uses": x\n        shell: bash\n' >>"$work/repo/.github/workflows/ci.yml"
expect_exit "quoted text in a plain value and quoted keys in a run script pass" 0

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
