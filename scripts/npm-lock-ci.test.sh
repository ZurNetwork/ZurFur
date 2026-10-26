#!/bin/sh
# Tests for scripts/npm-lock-ci.sh and the npm-lock job's inline step in
# .github/workflows/ci.yml, against scratch git repos with real test-merge
# commits. The tools are stubs committed in each repo, base and change copies
# told apart, and a stub `node`; each records its arguments, its environment
# (read from /proc, as the process received it) and which copy ran into
# $HOME/log, and exits with the code in $HOME/adv-exit or $HOME/floor-exit,
# or, for an allow-list-only run or a waivers parse (old and new the same
# file), $HOME/allow-exit or $HOME/parse-exit (0 when absent; 2 when a file
# they are given is missing, as the real tools fail closed).
# HOME is the one control channel, since the driver passes it through its
# clean environments. Run directly, or via `just scripts-test`. Needs git,
# bash and a Linux /proc.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$script_dir/.." && pwd)"
driver="$script_dir/npm-lock-ci.sh"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

failures=0
repo="$work/repo"
home="$work/home"
mkdir -p "$work/bin" "$home" "$work/runner"

ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1: $2"
    failures=$((failures + 1))
}

# The stub node: records its environment's names and PATH, then runs the
# "lock-review.mjs" it is given, a shell script here, with its arguments.
cat >"$work/bin/node" <<'STUB'
#!/bin/sh
names="$(tr '\0' '\n' </proc/$$/environ | cut -d= -f1 | sort | paste -sd, -)"
printf 'node env=%s path=%s\n' "$names" "$PATH" >>"$HOME/log"
exec sh "$@"
STUB
chmod +x "$work/bin/node"

# passing_tools: the change's tools, each recording that it ran and passing.
passing_tools() {
    printf '#!/bin/sh\necho "advisories copy=pr" >>"$HOME/log"\nexit 0\n' >"$repo/scripts/npm-advisories.sh"
    printf 'echo "floor copy=pr" >>"$HOME/log"\nexit 0\n' >"$repo/frontend/lock-review.mjs"
    echo "install-check pr" >"$repo/frontend/install-check.mjs"
}

# stub_tools <copy>: writes the three tools into the repo, each naming <copy>.
stub_tools() {
    mkdir -p "$repo/scripts" "$repo/frontend"
    cat >"$repo/scripts/npm-advisories.sh" <<STUB
#!/bin/sh
names="\$(tr '\\0' '\\n' </proc/\$\$/environ | cut -d= -f1 | sort | paste -sd, -)"
if [ "\$1" = --allow-list-only ]; then
    printf 'allow-list copy=$1 env=%s mode=%s args=%s %s %s %s frontend=%s lexicons=%s\n' "\$names" "\$NPM_ADVISORIES_MODE" "\$2" "\$3" "\$4" "\$5" "\$(cat "\$3" 2>/dev/null || echo MISSING)" "\$(cat "\$5" 2>/dev/null || echo MISSING)" >>"\$HOME/log"
    [ -f "\$3" ] && [ -f "\$5" ] || exit 2
    exit "\$(cat "\$HOME/allow-exit" 2>/dev/null || echo 0)"
fi
printf 'advisories copy=$1 env=%s mode=%s\n' "\$names" "\$NPM_ADVISORIES_MODE" >>"\$HOME/log"
printf 'advisories args=%s %s %s %s frontend=%s lexicons=%s\n' "\$1" "\$2" "\$3" "\$4" "\$(cat "\$2")" "\$(cat "\$4")" >>"\$HOME/log"
exit "\$(cat "\$HOME/adv-exit")"
STUB
    cat >"$repo/frontend/lock-review.mjs" <<STUB
dir="\$(dirname "\$0")"
if [ "\$1" = "\$2" ]; then
    printf 'waivers-parse copy=$1 old=%s flags=%s %s %s file=%s parsed=%s\n' "\$(tr '\\n' '|' <"\$1")" "\$3" "\$4" "\$5" "\$6" "\$(cat "\$6" 2>/dev/null || echo MISSING)" >>"\$HOME/log"
    [ -f "\$6" ] || exit 2
    exit "\$(cat "\$HOME/parse-exit" 2>/dev/null || echo 0)"
fi
printf 'floor copy=$1 old=%s new=%s flags=%s %s %s sibling=%s waivers=%s\n' "\$(tr '\\n' '|' <"\$1")" "\$2" "\$3" "\$4" "\$5" "\$(cat "\$dir/install-check.mjs")" "\$(cat "\$6")" >>"\$HOME/log"
exit "\$(cat "\$HOME/floor-exit")"
STUB
    echo "install-check $1" >"$repo/frontend/install-check.mjs"
}

# write <path> <content>: one file in the repo.
write() {
    mkdir -p "$(dirname "$repo/$1")"
    printf '%s\n' "$2" >"$repo/$1"
}

g() { git -C "$repo" -c user.email=t@example.com -c user.name=t "$@" >/dev/null 2>&1; }

# new_base [no-driver]: a base commit on main holding the real driver (unless
# no-driver), stub tools marked base, lockfiles, waivers and ignore files,
# then a `pr` branch.
new_base() {
    rm -rf "$repo"
    mkdir -p "$repo/scripts"
    if [ "${1:-}" != no-driver ]; then
        cp "$driver" "$repo/scripts/npm-lock-ci.sh"
    fi
    stub_tools base
    write frontend/yarn.lock "yarn old"
    write lexicons/package-lock.json "npm old"
    write lock-age-waivers.json "waivers base"
    write osv-scanner.frontend.toml "frontend ignores base"
    write osv-scanner.lexicons.toml "lexicons ignores base"
    git -C "$repo" init -q -b main
    g add -A
    g commit -m base
    base_sha="$(git -C "$repo" rev-parse HEAD)"
    g checkout -b pr
}

# change_ignores: the change's own ignore files, marked pr.
change_ignores() {
    write osv-scanner.frontend.toml "frontend ignores pr"
    write osv-scanner.lexicons.toml "lexicons ignores pr"
}

# pr_commit: commits the change, then builds GitHub's test merge (base first,
# the change second) and checks it out.
pr_commit() {
    g add -A
    g commit -m change
    g checkout -q --detach "$base_sha"
    g merge --no-ff -m merge pr
}

# exits <adv> <floor> [<allow-list> <parse>]: the stub tools' exit codes.
exits() {
    echo "$1" >"$home/adv-exit"
    echo "$2" >"$home/floor-exit"
    echo "${3:-0}" >"$home/allow-exit"
    echo "${4:-0}" >"$home/parse-exit"
}

# The npm-lock job's `run: |` block, from the workflow, de-indented.
awk '
    /^  npm-lock:/ { job = 1; next }
    job && /^  [^[:space:]#]/ { job = 0 }
    job && !inrun && /^[[:space:]]+run: \|[[:space:]]*$/ { inrun = 1; match($0, /^ */); ind = RLENGTH; next }
    inrun {
        if ($0 !~ /^[[:space:]]*$/) { match($0, /^ */); if (RLENGTH <= ind) { inrun = 0; next } }
        print substr($0, ind + 3)
    }
' "$root/.github/workflows/ci.yml" >"$work/step.sh"
if grep -qF 'sh "$driver" "$base" "$NPM_LOCK_MODE"' "$work/step.sh"; then
    ok "the inline step is extracted from the npm-lock job"
else
    fail "the inline step is extracted from the npm-lock job" "got: $(cat "$work/step.sh")"
fi

# The leaked variables the clean environments must drop.
leaks="NODE_OPTIONS=--require=/x NODE_USE_ENV_PROXY=1 NODE_EXTRA_CA_CERTS=/x NODE_TLS_REJECT_UNAUTHORIZED=0 HTTPS_PROXY=http://proxy.invalid"

# step <event> <base sha> <mode> [env assignments…]: runs the inline step in
# the repo as GitHub does (bash -eo pipefail), sets status and out.
step() {
    event="$1"
    sha="$2"
    step_mode="$3"
    shift 3
    : >"$home/log"
    rm -f "$work/runner/npm-lock-ci.sh"
    status=0
    # shellcheck disable=SC2086 # $leaks is a list of assignments
    out="$(cd "$repo" && env HOME="$home" PATH="$work/bin:$PATH" RUNNER_TEMP="$work/runner" \
        GITHUB_EVENT_NAME="$event" BASE_SHA="$sha" NPM_LOCK_MODE="$step_mode" $leaks "$@" \
        bash --noprofile --norc -eo pipefail "$work/step.sh" 2>&1)" || status=$?
    log="$(cat "$home/log")"
}

# run_driver <event> <args…>: runs this checkout's driver directly, sets status and out.
run_driver() {
    event="$1"
    shift
    : >"$home/log"
    status=0
    # shellcheck disable=SC2086 # $leaks is a list of assignments
    out="$(cd "$repo" && env HOME="$home" PATH="$work/bin:$PATH" GITHUB_EVENT_NAME="$event" $leaks \
        sh "$driver" "$@" 2>&1)" || status=$?
    log="$(cat "$home/log")"
}

# expect_status <name> <code>
expect_status() {
    if [ "$status" = "$2" ]; then
        ok "$1"
    else
        fail "$1" "exit $status, wanted $2; output: $out; log: $log"
    fi
}

# expect_in <name> <haystack> <needle>
expect_in() {
    if printf '%s\n' "$2" | grep -qF -- "$3"; then
        ok "$1"
    else
        fail "$1" "lacks '$3' in: $2"
    fi
}

# expect_not_in <name> <haystack> <needle>
expect_not_in() {
    if printf '%s\n' "$2" | grep -qF -- "$3"; then
        fail "$1" "has '$3' in: $2"
    else
        ok "$1"
    fi
}

# --- which tools run ----------------------------------------------------------

# The change rewrites the driver and every tool to pass; the base's copies
# still judge it.
new_base
printf '#!/bin/sh\nexit 0\n' >"$repo/scripts/npm-lock-ci.sh"
passing_tools
change_ignores
pr_commit
exits 1 0
step pull_request "$base_sha" block
expect_status "a change whose driver and tools say exit 0 still fails on the base's tools" 1
expect_in "the base's advisory tool ran" "$log" "advisories copy=base"
expect_in "the base's floor ran" "$log" "floor copy=base"
expect_in "the floor's sibling install-check.mjs is the base's" "$log" "sibling=install-check base"
expect_not_in "no tool of the change's ran" "$log" "copy=pr"
expect_in "the driver names each tool's source" "$out" "tool frontend/lock-review.mjs: from the base"
exits 0 0
step pull_request "$base_sha" block
expect_status "with the base's tools passing, the change passes" 0

# A base without the driver: the change's own driver and tools, with warnings.
new_base no-driver
cp "$driver" "$repo/scripts/npm-lock-ci.sh"
stub_tools pr
pr_commit
exits 0 0
step pull_request "$base_sha" block
expect_status "a base without the driver runs the change's" 0
expect_in "the step warns that it runs the change's driver" "$out" "::warning::npm-lock: the base has no scripts/npm-lock-ci.sh; running this change's own"
expect_in "the driver warns that it uses the change's tools" "$out" "::warning::npm-lock: the base has no scripts/npm-lock-ci.sh; using this change's own tools"
expect_in "the change's advisory tool ran" "$log" "advisories copy=pr"
expect_in "the change's floor ran" "$log" "floor copy=pr"
expect_not_in "no base tool ran" "$log" "copy=base"

# A base whose tree can't be read is not a base without the driver.
new_base no-driver
cp "$driver" "$repo/scripts/npm-lock-ci.sh"
pr_commit
tree="$(git -C "$repo" rev-parse "$base_sha^{tree}")"
rm -f "$repo/.git/objects/$(printf '%s' "$tree" | cut -c1-2)/$(printf '%s' "$tree" | cut -c3-)"
exits 0 0
step pull_request "$base_sha" block
expect_status "a base whose tree can't be read fails rather than running the change's driver" 1
expect_in "the step reports the failed read" "$out" "::error::npm-lock: can't read the base's tree"
expect_not_in "the step does not fall back to the change's driver" "$out" "running this change's own"
expect_not_in "nothing ran on an unreadable base" "$log" "copy="

# --- base verification ----------------------------------------------------------

new_base
echo "change" >"$repo/change.md"
pr_commit
exits 0 0
step pull_request "0123456789abcdef0123456789abcdef01234567" block
expect_status "the step: a base commit missing from the checkout exits 1" 1
step pull_request "$(printf '%s' "$base_sha" | tr 'a-f' 'A-F')" block
expect_status "the step: an uppercase (malformed) SHA exits 1" 1
step pull_request "" block
expect_status "the step: an empty base SHA exits 1" 1
run_driver pull_request "0123456789abcdef0123456789abcdef01234567" block
expect_status "the driver: a base commit missing from the checkout exits 1" 1
run_driver pull_request "${base_sha}0" block
expect_status "the driver: a malformed SHA exits 1" 1
run_driver workflow_dispatch "$base_sha" block
expect_status "the driver: another event exits 1" 1
run_driver pull_request "$base_sha" sometimes
expect_status "the driver: an unknown mode is bad usage" 2
expect_not_in "nothing ran on a failed verification" "$log" "copy="

# A pull request whose HEAD^1 isn't the base it names: a side commit holding
# the driver, so the step runs that copy and it refuses.
side_sha="$(git -C "$repo" commit-tree -p "$base_sha" -m side "$base_sha^{tree}")"
step pull_request "$side_sha" block
expect_status "a pull request whose HEAD^1 isn't the base exits 1" 1
expect_in "it says why" "$out" "is not the base $side_sha"
expect_not_in "nothing ran when HEAD^1 isn't the base" "$log" "copy="

# A merge group (and a push): HEAD is one more commit on the base, as the
# merge queue builds its group; the base must be an ancestor of HEAD.
new_base
echo "change" >"$repo/change.md"
g add -A
g commit -m change
step merge_group "$base_sha" block
expect_status "a merge group whose base is an ancestor passes" 0
expect_in "a merge group verifies ancestry" "$out" "is an ancestor of HEAD"
loose="$(git -C "$repo" -c user.email=t@example.com -c user.name=t commit-tree -m loose "$base_sha^{tree}")"
step merge_group "$loose" block
expect_status "a merge group whose base isn't an ancestor exits 1" 1
expect_in "it says why" "$out" "is not an ancestor of HEAD"

# A push: the base is HEAD^1, whatever BASE_SHA says.
step push "" block
expect_status "a push uses HEAD^1" 0
expect_in "a push verifies HEAD^1" "$out" "base verified: HEAD^1 is $base_sha"
step push "$loose" block
expect_status "a push ignores the event's base SHA" 0

# --- waivers ------------------------------------------------------------------------

new_base
write lock-age-waivers.json "waivers pr"
pr_commit
exits 0 0
step pull_request "$base_sha" block
expect_in "waivers come from the base" "$log" "waivers=waivers base"
expect_not_in "the change's waivers are not used by the floor" "$log" "waivers=waivers pr"
expect_in "changed waivers are parsed by the base's lock-review over an empty diff" "$log" \
    "waivers-parse copy=base old=yarn old| flags=--min-age-days 7 --waivers file=lock-age-waivers.json parsed=waivers pr"
expect_in "the driver says it parses the change's waivers" "$out" "this change's lock-age-waivers.json differs from the base's; parsing it"
exits 0 0 0 2
step pull_request "$base_sha" report
expect_status "a waiver-only change whose waivers don't parse exits 2, even in report mode" 2
expect_in "the parse's exit is recorded" "$out" "npm-lock: this change's waivers: exit 2"
exits 0 0

new_base
echo "change" >"$repo/change.md"
pr_commit
step pull_request "$base_sha" block
expect_status "unchanged waivers pass" 0
expect_not_in "unchanged waivers are not parsed again" "$log" "waivers-parse"

new_base
g rm -q lock-age-waivers.json
pr_commit
step pull_request "$base_sha" report
expect_status "a change that deletes the waivers fails closed" 2
expect_in "the deleted waivers are what the parse was given" "$log" "file=lock-age-waivers.json parsed=MISSING"

new_base
g rm -q lock-age-waivers.json
g commit -m "no waivers"
base_sha="$(git -C "$repo" rev-parse HEAD)"
write lock-age-waivers.json "waivers pr"
pr_commit
step pull_request "$base_sha" block
expect_status "a base without waivers passes on the change's" 0
expect_in "a base without waivers uses the change's" "$log" "waivers=waivers pr"
expect_in "with a warning" "$out" "::warning::npm-lock: the base has no lock-age-waivers.json; using this change's"
expect_not_in "a base without waivers: no second parse" "$log" "waivers-parse"

# --- ignores --------------------------------------------------------------------------

new_base
change_ignores
write frontend/yarn.lock "yarn new"
pr_commit
step pull_request "$base_sha" block
expect_in "a changed frontend lockfile takes the base's frontend ignores" "$log" "frontend=frontend ignores base"
expect_in "an unchanged lexicons lockfile takes the change's lexicons ignores" "$log" "lexicons=lexicons ignores pr"
expect_in "a base's ignore file used: the change's own pass the base's allow-list, under env -i" "$log" \
    "allow-list copy=base env=HOME,NPM_ADVISORIES_MODE,PATH mode=block args=--frontend-config osv-scanner.frontend.toml --lexicons-config osv-scanner.lexicons.toml frontend=frontend ignores pr lexicons=lexicons ignores pr"
exits 0 0 2 0
step pull_request "$base_sha" block
expect_status "a change's own ignore file that breaks the allow-list exits 2" 2
expect_in "the allow-list's exit is recorded" "$out" "npm-lock: allow-list of this change's ignore files: exit 2"
exits 0 0
step pull_request "$base_sha" report
expect_in "the allow-list runs in the scan's mode" "$log" "allow-list copy=base env=HOME,NPM_ADVISORIES_MODE,PATH mode=report"

new_base
change_ignores
write lexicons/package-lock.json "npm new"
pr_commit
step pull_request "$base_sha" block
expect_in "a changed lexicons lockfile takes the base's lexicons ignores" "$log" "lexicons=lexicons ignores base"
expect_in "an unchanged frontend lockfile takes the change's frontend ignores" "$log" "frontend=frontend ignores pr"
expect_in "a changed lexicons lockfile: the change's ignore files pass the allow-list" "$log" \
    "allow-list copy=base env=HOME,NPM_ADVISORIES_MODE,PATH mode=block args=--frontend-config osv-scanner.frontend.toml --lexicons-config osv-scanner.lexicons.toml frontend=frontend ignores pr lexicons=lexicons ignores pr"

new_base
change_ignores
pr_commit
step pull_request "$base_sha" block
expect_in "no lockfile change: the change's frontend ignores" "$log" "frontend=frontend ignores pr"
expect_in "no lockfile change: the change's lexicons ignores" "$log" "lexicons=lexicons ignores pr"
expect_in "the advisory tool gets both configs" "$log" "advisories args=--frontend-config"
expect_not_in "no lockfile change: no separate allow-list run" "$log" "allow-list copy="

new_base
g rm -q osv-scanner.frontend.toml
write frontend/yarn.lock "yarn new"
pr_commit
step pull_request "$base_sha" block
expect_status "a lockfile change that deletes its own ignore file fails closed" 2
expect_in "the deleted ignore file is what the allow-list was given" "$log" "frontend=MISSING"

new_base
g rm -q osv-scanner.frontend.toml
g commit -m "no frontend ignores"
base_sha="$(git -C "$repo" rev-parse HEAD)"
change_ignores
write frontend/yarn.lock "yarn new"
pr_commit
step pull_request "$base_sha" block
expect_in "a changed lockfile whose base has no ignores takes the change's" "$log" "frontend=frontend ignores pr"
expect_not_in "no base ignore file used: no separate allow-list run" "$log" "allow-list copy="

# --- old lockfiles ----------------------------------------------------------------------

new_base
g rm -q frontend/yarn.lock
write frontend/web/yarn.lock "yarn web old"
g add -A
g commit -m "the old layout"
base_sha="$(git -C "$repo" rev-parse HEAD)"
write frontend/yarn.lock "yarn new"
pr_commit
step pull_request "$base_sha" block
expect_in "the old frontend lockfile at frontend/web/yarn.lock" "$log" "old=yarn web old| new=frontend/yarn.lock"

new_base
g rm -q frontend/yarn.lock lexicons/package-lock.json
g commit -m "no lockfiles"
base_sha="$(git -C "$repo" rev-parse HEAD)"
write frontend/yarn.lock "yarn new"
write lexicons/package-lock.json "npm new"
pr_commit
step pull_request "$base_sha" block
expect_in "no old frontend lockfile: an empty yarn v1 file" "$log" "old=# THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.|# yarn lockfile v1| new=frontend/yarn.lock"
expect_in "no old lexicons lockfile: an empty npm v3 file" "$log" 'old={"lockfileVersion": 3, "packages": {"": {}}}| new=lexicons/package-lock.json'

# --- the clean environments ---------------------------------------------------------------

new_base
echo "change" >"$repo/change.md"
pr_commit
exits 0 0
step pull_request "$base_sha" report
expect_in "node sees only PATH and HOME" "$log" "node env=HOME,PATH path=$work/bin:/usr/bin:/bin"
expect_in "the advisory tool sees only HOME, NPM_ADVISORIES_MODE and PATH" "$log" "advisories copy=base env=HOME,NPM_ADVISORIES_MODE,PATH mode=report"
expect_in "the floor runs with --min-age-days 7 and --waivers" "$log" "flags=--min-age-days 7 --waivers"
step pull_request "$base_sha" block
expect_in "block mode reaches the advisory tool" "$log" "mode=block"

# --- the worst code wins ------------------------------------------------------------------

exits 1 2
step pull_request "$base_sha" block
expect_status "advisories 1, floor 2: exit 2" 2
floors="$(printf '%s\n' "$log" | grep -c '^floor copy=base' || true)"
if [ "$floors" = 2 ]; then
    ok "both lockfiles' floors run after the advisory check fails"
else
    fail "both lockfiles' floors run after the advisory check fails" "$floors floor runs; log: $log"
fi
exits 3 1
step pull_request "$base_sha" block
expect_status "advisories 3, floor 1: exit 3" 3
exits 0 1
step pull_request "$base_sha" report
expect_status "report mode leaves the floor blocking" 1
exits 4 0
step pull_request "$base_sha" block
expect_status "advisories 4, floors 0: exit 4" 4

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all npm-lock-ci tests passed"
