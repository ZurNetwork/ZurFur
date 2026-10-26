#!/bin/sh
# Pins the npm lockfile checks' wiring: the npm-lock job's shape and its inline
# step in CI (the base's driver and scripts folder read only as a regular file
# and a folder), the jobs that wait for it, the driver's tools, clean
# environments, floor and checks of the change's own waivers, ignore files and
# licence list (both read from HEAD), its read set (regular files only, at the
# base and at HEAD), the base's licence list for every floor, the daily npm
# advisory and licence jobs, the guard's tests in the gate, gate-light and CI,
# `just npm-lock-review`'s licence list, the cargo config check in `just gate`
# and the hooks job, and `just gate` ending on the licence sweep and then the
# advisory scan. Each line here is one a quiet edit could drop or loosen. Run
# directly, or via `just scripts-test`.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
ci="$root/.github/workflows/ci.yml"
failures=0

ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1: $2"
    failures=$((failures + 1))
}

# job <workflow> <job id>: the job's body, its lines up to the next job.
job() {
    awk -v id="$2" '
        $0 == "  " id ":" { inside = 1; next }
        inside && /^  [^[:space:]#]/ { inside = 0 }
        inside && /^[^[:space:]#]/ { inside = 0 }
        inside { print }
    ' "$1"
}

# expect_line <name> <haystack> <line>: one whole line, indentation aside, is <line>.
expect_line() {
    # ENVIRON, not -v: older awks (the runner's gawk 5.2) drop a trailing backslash from -v.
    if printf '%s\n' "$2" | want="$3" awk '
        BEGIN { want = ENVIRON["want"] }
        { sub(/^[[:space:]]+/, ""); sub(/[[:space:]]+$/, "") }
        $0 == want { found = 1 }
        END { exit !found }
    '; then
        ok "$1"
    else
        fail "$1" "no line '$3'"
    fi
}

# expect_count <name> <haystack> <line> <count>: exactly <count> whole lines,
# indentation aside, are <line>.
expect_count() {
    got="$(printf '%s\n' "$2" | want="$3" awk '
        BEGIN { want = ENVIRON["want"] }
        { sub(/^[[:space:]]+/, ""); sub(/[[:space:]]+$/, "") }
        $0 == want { n++ }
        END { print n + 0 }
    ')"
    if [ "$got" = "$4" ]; then
        ok "$1"
    else
        fail "$1" "$got lines '$3', wanted $4"
    fi
}

# expect_none <name> <haystack> <extended regex>: no line matches.
expect_none() {
    found="$(printf '%s\n' "$2" | grep -inE -- "$3" || true)"
    if [ -z "$found" ]; then
        ok "$1"
    else
        fail "$1" "found: $found"
    fi
}

# --- the npm-lock job ----------------------------------------------------------

npm_lock="$(job "$ci" npm-lock)"
if [ -z "$npm_lock" ]; then
    fail "ci.yml has an npm-lock job" "none found"
fi

expect_line "npm-lock needs the changes job" "$npm_lock" "needs: changes"
expect_line "npm-lock runs unless both code and npmlock are exactly false" "$npm_lock" \
    "if: \${{ !cancelled() && (needs.changes.outputs.code != 'false' || needs.changes.outputs.npmlock != 'false') }}"

# Its steps: exactly checkout, setup-node, then the driver step, in that order.
steps="$(printf '%s\n' "$npm_lock" | awk '/^      - / { sub(/^      - /, ""); print }' | tr '\n' '|')"
want_steps="uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1|uses: actions/setup-node@949feb2413d6458794dcd2491c4babbbce0c15c1 # v7.1.0|name: The advisory check, the 7-day floor and the licence rule, with the base's tools|"
if [ "$steps" = "$want_steps" ]; then
    ok "npm-lock's three steps: checkout, setup-node, the driver step"
else
    fail "npm-lock's three steps: checkout, setup-node, the driver step" "got '$steps'"
fi
expect_line "npm-lock's checkout stores no credentials" "$npm_lock" "persist-credentials: false"
expect_line "npm-lock's checkout fetches the base (full history for a merge group)" "$npm_lock" \
    "fetch-depth: \${{ github.event_name == 'merge_group' && '0' || '2' }}"
expect_line "npm-lock's node is 24" "$npm_lock" "node-version: '24'"
expect_line "npm-lock switches off setup-node's automatic package cache" "$npm_lock" "package-manager-cache: false"
# Every other cache key line, any spelling or quoting, flow mappings too.
other_caches="$(printf '%s\n' "$npm_lock" | grep -vE '^[[:space:]]*#' |
    grep -iE "cache[a-z_-]*[\"']?[[:space:]]*:" | grep -vxE '[[:space:]]*package-manager-cache: false[[:space:]]*' || true)"
if [ -z "$other_caches" ]; then
    ok "npm-lock sets no other cache key"
else
    fail "npm-lock sets no other cache key" "found: $other_caches"
fi
expect_none "npm-lock runs no cargo, cargo-deny or yarn" "$npm_lock" 'cargo|yarn'

# The inline step: the base, its commit, its place (HEAD^1 or an ancestor),
# then the base's copy or the fallback.
expect_line "the base SHA comes from the event through env" "$npm_lock" \
    "BASE_SHA: \${{ github.event.pull_request.base.sha || github.event.merge_group.base_sha }}"
expect_line "a push's base is HEAD^1" "$npm_lock" \
    "base=\$(git rev-parse --verify --quiet 'HEAD^1') || base=\"\""
expect_line "the base SHA must be 40 lowercase hex" "$npm_lock" \
    "if ! printf '%s' \"\$base\" | grep -qE '^[0-9a-f]{40}\$'; then"
expect_line "the base commit must be present" "$npm_lock" \
    "if ! git cat-file -e \"\$base^{commit}\"; then"
expect_line "a pull request or a push checks HEAD^1 in the step" "$npm_lock" "pull_request | push)"
expect_line "the step reads HEAD^1" "$npm_lock" \
    "parent1=\$(git rev-parse --verify --quiet 'HEAD^1') || parent1=\"\""
expect_line "the step's HEAD^1 must be the base" "$npm_lock" "if [ \"\$parent1\" != \"\$base\" ]; then"
expect_line "the step refuses a HEAD^1 that isn't the base" "$npm_lock" \
    "echo \"::error::npm-lock: HEAD^1 (\$parent1) is not the base \$base\""
expect_line "a merge group checks ancestry in the step" "$npm_lock" "merge_group)"
expect_line "the step's merge group base must be an ancestor of HEAD" "$npm_lock" \
    "if ! git merge-base --is-ancestor \"\$base\" HEAD; then"
expect_line "the step refuses a base that isn't an ancestor" "$npm_lock" \
    "echo \"::error::npm-lock: the merge group's base \$base is not an ancestor of HEAD\""
expect_line "the step refuses any other event" "$npm_lock" \
    "echo \"::error::npm-lock: not a pull request, a merge group or a push (GITHUB_EVENT_NAME '\$GITHUB_EVENT_NAME')\""
expect_line "a failed tree read fails, apart from an absent driver" "$npm_lock" \
    "if ! listing=\$(git ls-tree --full-tree \"\$base\" -- scripts/npm-lock-ci.sh); then"
expect_line "the base's driver is extracted" "$npm_lock" \
    "if ! git show \"\$base:scripts/npm-lock-ci.sh\" > \"\$RUNNER_TEMP/npm-lock-ci.sh\"; then"
expect_line "the base's driver must be a regular file" "$npm_lock" "\"100644 blob \"*) ;;"
expect_line "the step refuses a base driver that isn't a regular file" "$npm_lock" \
    "echo \"::error::npm-lock: the base's scripts/npm-lock-ci.sh is mode \${listing%% *}, not a regular file (100644)\""
expect_line "a base without the driver: its scripts entry is read" "$npm_lock" \
    "if ! folder=\$(git ls-tree --full-tree \"\$base\" -- scripts); then"
expect_line "the base's scripts entry must be absent or a folder" "$npm_lock" "case \"\$folder\" in"
expect_line "the step's folder test" "$npm_lock" "\"\" | \"040000 tree \"*) ;;"
expect_line "the step refuses a base scripts entry that isn't a folder" "$npm_lock" \
    "echo \"::error::npm-lock: the base's scripts is mode \${folder%% *}, not a folder (040000)\""
expect_line "the base's copy runs when it has one" "$npm_lock" "driver=\"\$RUNNER_TEMP/npm-lock-ci.sh\""
expect_line "only a base without the driver runs the change's" "$npm_lock" "driver=scripts/npm-lock-ci.sh"
expect_line "the fallback warns" "$npm_lock" \
    "echo \"::warning::npm-lock: the base has no scripts/npm-lock-ci.sh; running this change's own\""
expect_line "the driver gets the base and the mode" "$npm_lock" "sh \"\$driver\" \"\$base\" \"\$NPM_LOCK_MODE\""
expect_line "advisories report only when npmlock is exactly false" "$npm_lock" \
    "NPM_LOCK_MODE: \${{ needs.changes.outputs.npmlock == 'false' && 'report' || 'block' }}"

changes="$(job "$ci" changes)"
expect_line "the changes job exports npmlock" "$changes" "npmlock: \${{ steps.detect.outputs.npmlock }}"

# --- the jobs that wait for it -------------------------------------------------------

for waiter in web ui contract; do
    body="$(job "$ci" "$waiter")"
    expect_line "$waiter needs npm-lock" "$body" "needs: [changes, npm-lock]"
    expect_line "$waiter runs only after npm-lock succeeds" "$body" \
        "if: \${{ !cancelled() && needs.changes.outputs.code != 'false' && needs.npm-lock.result == 'success' }}"
done

# --- the driver -------------------------------------------------------------------------

driver="$(cat "$root/scripts/npm-lock-ci.sh")"
expect_line "the advisory check runs under env -i" "$driver" \
    "env -i PATH=/usr/bin:/bin HOME=\"\${HOME:-/nonexistent}\" NPM_ADVISORIES_MODE=\"\$mode\" \\"
expect_line "the floor runs under env -i" "$driver" \
    "env -i PATH=\"\$node_dir:/usr/bin:/bin\" HOME=\"\${HOME:-/nonexistent}\" \\"
expect_line "the floor is 7 days, with the chosen waivers" "$driver" "--min-age-days 7 --waivers \"\$waivers\""
expect_line "the change's own ignore files are checked when the scan used a base's" "$driver" \
    "if [ \"\$base_ignores\" = 1 ]; then"
expect_line "the base's advisory tool runs its allow-list alone" "$driver" \
    "sh \"\$tools/scripts/npm-advisories.sh\" --allow-list-only \\"
expect_line "the allow-list gets the change's own ignore files" "$driver" \
    "--frontend-config osv-scanner.frontend.toml --lexicons-config osv-scanner.lexicons.toml"
expect_line "the change's waivers are parsed when HEAD's differ from the base's" "$driver" \
    "if [ \"\$own_waivers_ok\" = 1 ] && [ \"\$waivers\" != \"\$own_waivers\" ] && ! cmp -s \"\$waivers\" \"\$own_waivers\"; then"
expect_line "the base's lock-review parses them over an empty diff" "$driver" \
    "\"\$node_bin\" \"\$tools/frontend/lock-review.mjs\" \"\$old_frontend\" \"\$old_frontend\" \\"
expect_line "the parse gets HEAD's copy of the change's waivers" "$driver" "--min-age-days 7 --waivers \"\$own_waivers\""
expect_line "the driver's tools include licences.mjs" "$driver" \
    "TOOLS=\"scripts/npm-advisories.sh frontend/lock-review.mjs frontend/install-check.mjs frontend/licences.mjs\""
expect_line "a base with the driver must hold the licence list" "$driver" "if ! base_has npm-licences.json; then"
expect_line "a base without the list fails" "$driver" \
    "fail \"the base has scripts/npm-lock-ci.sh but no npm-licences.json, so no licence list to judge this change by\""
expect_line "the list is extracted from the base" "$driver" "from_base npm-licences.json \"\$licences\""
expect_count "both floors and the waivers parse get the list" "$driver" "--licences \"\$licences\" \\" 2
expect_line "the change's list is swept when HEAD's differs from the base's" "$driver" \
    "if [ \"\$own_licences_ok\" = 1 ] && [ \"\$licences\" != \"\$own_licences\" ] && ! cmp -s \"\$licences\" \"\$own_licences\"; then"
expect_line "the base's lock-review sweeps both lockfiles against HEAD's copy of the change's list" "$driver" \
    "\"\$node_bin\" \"\$tools/frontend/lock-review.mjs\" --licence-sweep --licences \"\$own_licences\" frontend/yarn.lock lexicons/package-lock.json"
expect_line "the sweep's exit is recorded" "$driver" "record \"this change's licence list\" \"\$?\""

# Regular files only: the read set, the base's entries and HEAD's.
expect_line "the data a run reads from its base" "$driver" \
    "DATA=\"frontend/yarn.lock frontend/web/yarn.lock lexicons/package-lock.json lock-age-waivers.json npm-licences.json osv-scanner.frontend.toml osv-scanner.lexicons.toml\""
expect_line "the read set is the driver, the tools and the data" "$driver" \
    "READ_SET=\"scripts/npm-lock-ci.sh \$TOOLS \$DATA\""
expect_line "each entry is read one path per ls-tree call" "$driver" \
    "listing=\"\$(git ls-tree --full-tree \"\$1\" -- \"\$2\")\" || return 1"
expect_line "each parent is read one path per ls-tree call" "$driver" \
    "listing=\"\$(git ls-tree --full-tree \"\$1\" -- \"\$parent\")\" || return 1"
expect_line "a parent must be absent or a folder" "$driver" "\"\" | \"040000 tree \"*) ;;"
expect_line "an entry must be a regular file" "$driver" "\"100644 blob \"*) present=1 ;;"
expect_line "the base reads only paths in the read set" "$driver" "*) fail \"\$1 is not in the driver's read set\" ;;"
expect_line "the base's entry is checked before each read" "$driver" \
    "entry_at \"\$base\" \"\$1\" || fail \"can't read the base's tree at \$1\""
expect_line "a base entry that isn't a regular file fails" "$driver" "fail \"the base's \$problem\""
expect_line "every base read goes through the check" "$driver" "base_has \"\$1\" || fail \"the base has no \$1\""
expect_line "every path of the read set is checked at HEAD" "$driver" "for path in \$READ_SET; do"
expect_line "HEAD's entry is checked" "$driver" "entry_at HEAD \"\$path\" || fail \"can't read HEAD's tree at \$path\""
expect_line "a HEAD entry that isn't a regular file is a finding" "$driver" "record \"this change's \$path\" 1"
expect_line "HEAD's copy is read from its blob" "$driver" "git show \"HEAD:\$1\" >\"\$own\" || fail \"can't read \$1 from HEAD\""
expect_line "the change's licence list comes from HEAD" "$driver" "head_copy npm-licences.json"
expect_line "the change's waivers come from HEAD" "$driver" "head_copy lock-age-waivers.json"
expect_line "a base without the driver takes the change's tools from HEAD" "$driver" "head_copy \"\$tool\""

# --- the cargo config check ------------------------------------------------------------

gate_body="$(awk '
    /^gate:/ { inside = 1; next }
    inside && /^[^[:space:]]/ { inside = 0 }
    inside { print }
' "$root/Justfile")"
expect_line "just gate runs the cargo config check" "$gate_body" "just cargo-config-check"
expect_line "the hooks job runs the cargo config check" "$(job "$ci" hooks)" "- run: just cargo-config-check"

# --- the daily advisories and the gate ----------------------------------------------------

daily="$(job "$root/.github/workflows/advisories.yml" npm-advisories)"
expect_line "the daily workflow has an npm-advisories job" "$daily" "name: npm-advisories"
expect_line "the daily npm-advisories job runs the scan" "$daily" "- run: sh scripts/npm-advisories.sh"
expect_line "the advisories workflow runs daily" "$(cat "$root/.github/workflows/advisories.yml")" "- cron: '23 5 * * *'"

licences_job="$(job "$root/.github/workflows/advisories.yml" npm-licences)"
expect_line "the daily workflow has an npm-licences job" "$licences_job" "name: npm-licences"
expect_line "the daily npm-licences job stores no credentials" "$licences_job" "persist-credentials: false"
expect_line "the daily npm-licences job's node" "$licences_job" \
    "- uses: actions/setup-node@949feb2413d6458794dcd2491c4babbbce0c15c1 # v7.1.0"
expect_line "the daily npm-licences job switches off setup-node's package cache" "$licences_job" "package-manager-cache: false"
expect_line "the daily npm-licences job sweeps both lockfiles" "$licences_job" \
    "- run: env -i PATH=\"\$PATH\" HOME=\"\$HOME\" node frontend/lock-review.mjs --licence-sweep --licences npm-licences.json frontend/yarn.lock lexicons/package-lock.json"

# The last two lines of `just gate`, comments aside: the sweep, then the scan.
gate_last="$(awk '
    /^gate:/ { inside = 1; next }
    inside && /^[^[:space:]]/ { inside = 0 }
    inside && /^[[:space:]]+[^[:space:]#]/ { prev = last; last = $0 }
    END { sub(/^[[:space:]]+/, "", prev); sub(/^[[:space:]]+/, "", last); print prev "|" last }
' "$root/Justfile")"
if [ "$gate_last" = "just npm-licences|just npm-advisories" ]; then
    ok "just gate ends with just npm-licences, then just npm-advisories"
else
    fail "just gate ends with just npm-licences, then just npm-advisories" "its last two lines are '$gate_last'"
fi

licences_recipe="$(awk '
    /^npm-licences:/ { inside = 1; next }
    inside && /^[^[:space:]]/ { inside = 0 }
    inside { print }
' "$root/Justfile")"
expect_line "just npm-licences sweeps both lockfiles under env -i" "$licences_recipe" \
    "env -i PATH=\"\$PATH\" HOME=\"\$HOME\" node frontend/lock-review.mjs --licence-sweep --licences npm-licences.json frontend/yarn.lock lexicons/package-lock.json"

# --- the guard's tests and the manual review -----------------------------------------

guard_tests="node --test frontend/install-check.test.mjs frontend/lock-review.test.mjs frontend/licences.test.mjs"
expect_line "just gate runs the guard's tests, licences included" "$gate_body" "$guard_tests"
expect_line "gate-light runs the guard's tests, licences included" "$(cat "$root/scripts/gate-light.sh")" "step $guard_tests"
expect_line "CI's ui job runs the guard's tests, licences included" "$(job "$ci" ui)" "- run: $guard_tests"
review_body="$(awk '
    /^npm-lock-review / { inside = 1; next }
    inside && /^[^[:space:]]/ { inside = 0 }
    inside { print }
' "$root/Justfile")"
expect_line "just npm-lock-review passes the licence list" "$review_body" \
    "node frontend/lock-review.mjs \"\$tmp/\$1\" \"\$2\" --licences npm-licences.json --min-age-days 7 --waivers lock-age-waivers.json"

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all npm-lock pins hold"
