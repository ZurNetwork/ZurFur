#!/bin/sh
# Compares this machine's tools with the versions CI uses, and checks that the
# workflows agree with those pins. Run via `just tools-check`, which exports
# TYPOS_VERSION, CARGO_DENY_VERSION, BUF_VERSION, JUST_VERSION and
# NODES_VERSION from the Justfile.
#
#   tools-check.sh              both checks; local drift only warns
#   tools-check.sh --pins-only  the workflow check alone (what CI runs)
#
# Exits 1 only when the workflows disagree with the Justfile's pins, an action
# is not pinned to a full commit SHA, or a workflow breaks a CI safety rule
# (see ci_safety below). A local version that differs from CI's prints a
# warning and leaves the exit code alone.
set -eu

: "${TYPOS_VERSION:?run through just tools-check}"
: "${CARGO_DENY_VERSION:?run through just tools-check}"
: "${BUF_VERSION:?run through just tools-check}"
: "${JUST_VERSION:?run through just tools-check}"
: "${NODES_VERSION:?run through just tools-check}"

pins_only=0
case "${1:-}" in
    "") ;;
    --pins-only) pins_only=1 ;;
    *)
        echo "usage: tools-check.sh [--pins-only]" >&2
        exit 2
        ;;
esac

root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
workflows=".github/workflows"
# GitHub loads both extensions; workflow file names carry no spaces here.
workflow_files="$(find "$workflows" -maxdepth 1 -type f \( -name '*.yml' -o -name '*.yaml' \) | sort)"
errors=0

pin_error() {
    echo "tools-check: $1" >&2
    errors=$((errors + 1))
}

# Every `uses:` reference is `<action>@<40-hex SHA>` directly, then `# v<version>`.
# shellcheck disable=SC2086 # $workflow_files is a newline list without spaces
unpinned="$(grep -hE '^[[:space:]]*-?[[:space:]]*uses:' $workflow_files |
    grep -vE '^[[:space:]]*-?[[:space:]]*uses:[[:space:]]*[^[:space:]@#]+@[0-9a-f]{40}[[:space:]]+# v[0-9]' || true)"
if [ -n "$unpinned" ]; then
    pin_error "actions not pinned to a commit SHA with a '# v<version>' comment:"
    printf '  %s\n' "$unpinned" >&2
fi

# check_every_use <action> <required text> — each line using <action> carries <text>.
check_every_use() {
    # shellcheck disable=SC2086
    lines="$(grep -hF "$1@" $workflow_files || true)"
    if [ -z "$lines" ]; then
        pin_error "no workflow uses $1"
        return
    fi
    if printf '%s\n' "$lines" | grep -vqF "$2"; then
        pin_error "$1 is not pinned at '$2' everywhere (Justfile pin)"
    fi
}
check_every_use crate-ci/typos "# v$TYPOS_VERSION"
check_every_use EmbarkStudios/cargo-deny-action "(cargo-deny $CARGO_DENY_VERSION)"
# step_pins <action> <input> <value> — prints "<steps> <pinned>": how many
# steps use <action>, and how many of those set `<input>: <value>` in their own
# `with:` block. A comment or an `env:` entry never counts, so a pin that
# survives only there fails.
step_pins() {
    # shellcheck disable=SC2086 # $workflow_files is a newline list without spaces
    awk -v action="$1" -v key="$2" -v want="$3" -v q="'" '
        function indent(s) { match(s, /^ */); return RLENGTH }
        function close_step() {
            if (in_step) { steps++; if (found) ok++ }
            in_step = 0
        }
        FNR == 1 { close_step() }
        {
            line = $0
            if (in_step) {
                if (line ~ /^ *(#.*)?$/) next
                if (indent(line) >= key_ind) {
                    t = line
                    sub(/^ */, "", t)
                    if (indent(line) == key_ind) in_with = (t ~ /^with:[ \t]*(#.*)?$/)
                    if (in_with && indent(line) > key_ind && index(t, key ":") == 1) {
                        v = substr(t, length(key) + 2)
                        sub(/[ \t]+#.*$/, "", v)
                        gsub(/^[ \t]+|[ \t]+$/, "", v)
                        if (v ~ ("^" q ".*" q "$") || v ~ /^".*"$/) v = substr(v, 2, length(v) - 2)
                        if (v == want) found = 1
                    }
                    next
                }
                close_step()
            }
            if (match(line, /^ *(- +)?uses: */)) {
                if (index(substr(line, RLENGTH + 1), action "@") == 1) {
                    match(line, /uses:/)
                    key_ind = RSTART - 1
                    in_step = 1
                    in_with = 0
                    found = 0
                }
            }
        }
        END { close_step(); print steps + 0, ok + 0 }
    ' $workflow_files
}

# check_step_input <action> <input> <value> — at least one step uses <action>,
# and every such step sets <input> to <value>.
check_step_input() {
    counts="$(step_pins "$1" "$2" "$3")"
    steps="${counts% *}"
    pinned="${counts#* }"
    if [ "$steps" -eq 0 ]; then
        pin_error "no workflow uses $1"
    elif [ "$steps" != "$pinned" ]; then
        pin_error "$steps $1 step(s) but $pinned set $2: '$3' (Justfile pin)"
    fi
}
check_step_input bufbuild/buf-action version "$BUF_VERSION"
check_step_input extractions/setup-just just-version "$JUST_VERSION"
# No checkout leaves its token in .git/config for later steps to read.
check_step_input actions/checkout persist-credentials false

# ci_safety — prints one "<file>:<line>: <problem>" per break of the CI safety
# rules, in every workflow file:
#   - a top-level `permissions:` block holding `contents: read`, every entry
#     read or none; no `permissions:` below the top level (a job's);
#   - no `pull_request_target`, no `secrets.`/`secrets[` expression and no
#     `secrets:` key, outside YAML comments (a run script's comments count);
#   - every yarn install (bare `yarn`, or a yarn command whose subcommand is
#     install, add, remove, upgrade, upgrade-interactive, global, import or
#     link, whatever flags come first) carries --frozen-lockfile and
#     --ignore-scripts, stands alone in its step's `run:`, and sits between a
#     step running exactly `node frontend/install-check.mjs` and one running
#     exactly `node frontend/install-check.mjs --installed`; those three steps
#     set nothing but run, name and working-directory;
#   - every `run:` is a step's (a `defaults: run:` block aside), so a shape
#     this reader can't follow fails rather than hiding an install.
# Top level means column 0: GitHub's workflow keys sit there, so any
# `permissions:` indented under them is a job's (or deeper).
ci_safety() {
    # shellcheck disable=SC2086 # $workflow_files is a newline list without spaces
    awk -v q="'" '
        function indent(s) { match(s, /^ */); return RLENGTH }
        function trim(s) { gsub(/^[ \t]+|[ \t]+$/, "", s); return s }
        function squeeze(s) { gsub(/[ \t\n]+/, " ", s); return trim(s) }
        function bare_value(s) { sub(/[ \t]+#.*$/, "", s); return trim(s) }
        function problem(file, at, msg) { print file ":" at ": " msg }
        function unquote(w) { gsub("[\"" q "]", "", w); return w }
        function is_yarn(w) { w = unquote(w); return w == "yarn" || w == "yarnpkg" || w ~ /\/(yarn|yarnpkg)$/ }

        # yarn_scan <text> — sets n_cmds (shell commands in <text>), n_inst
        # (yarn installs among them) and inst_frozen/inst_ignore (whether
        # every install carries the flag).
        function yarn_scan(text,    lines, nl, i, ln, cmds, nc, j, c, w, nw, k, sub_cmd, f, g) {
            n_cmds = 0; n_inst = 0; inst_frozen = 1; inst_ignore = 1
            gsub(/\\\n/, " ", text)
            nl = split(text, lines, "\n")
            for (i = 1; i <= nl; i++) {
                ln = lines[i]
                if (match(ln, /(^|[ \t])#/)) ln = substr(ln, 1, RSTART - 1)
                gsub(/&&|\|\||[;|&()`]/, "\n", ln)
                nc = split(ln, cmds, "\n")
                for (j = 1; j <= nc; j++) {
                    c = trim(cmds[j])
                    if (c == "") continue
                    n_cmds++
                    nw = split(c, w, /[ \t]+/)
                    for (k = 1; k <= nw; k++) if (is_yarn(w[k])) break
                    if (k > nw) continue
                    sub_cmd = ""
                    for (k++; k <= nw; k++) {
                        if (unquote(w[k]) in VALUE_FLAGS) { k++; continue }
                        if (unquote(w[k]) ~ /^-/) continue
                        sub_cmd = unquote(w[k])
                        break
                    }
                    if (sub_cmd != "" && !(sub_cmd in INSTALLS)) continue
                    n_inst++
                    f = 0; g = 0
                    for (k = 1; k <= nw; k++) {
                        if (unquote(w[k]) == "--frozen-lockfile") f = 1
                        if (unquote(w[k]) == "--ignore-scripts") g = 1
                    }
                    if (!f) inst_frozen = 0
                    if (!g) inst_ignore = 0
                }
            }
        }

        function close_run() {
            if (in_run) run_text[run_step] = run_text[run_step] run_buf
            in_run = 0
        }

        # trio_keys <i> — whether step <i> sets only run, name and working-directory.
        function trio_keys(i,    ks, nk, j) {
            nk = split(step_keys[i], ks, " ")
            for (j = 1; j <= nk; j++)
                if (ks[j] != "run" && ks[j] != "name" && ks[j] != "working-directory") return 0
            return 1
        }

        function end_steps(    i, at) {
            close_run()
            if (!in_steps) return
            in_steps = 0
            for (i = 1; i <= n; i++) {
                if (!has_run[i]) continue
                yarn_scan(run_text[i])
                if (n_inst == 0) continue
                at = step_line[i]
                if (!inst_frozen) problem(blk_file, at, "a yarn install without --frozen-lockfile")
                if (!inst_ignore) problem(blk_file, at, "a yarn install without --ignore-scripts")
                if (n_cmds != 1 || has_run[i] != 1) problem(blk_file, at, "a yarn install that shares its step with other commands")
                if (i == 1 || !has_run[i - 1] || squeeze(run_text[i - 1]) != GUARD)
                    problem(blk_file, at, "a yarn install without `" GUARD "` as the step before it")
                else if (!trio_keys(i - 1))
                    problem(blk_file, step_line[i - 1], "the install guard step sets more than run, name and working-directory")
                if (i == n || !has_run[i + 1] || squeeze(run_text[i + 1]) != GUARD " --installed")
                    problem(blk_file, at, "a yarn install without `" GUARD " --installed` as the step after it")
                else if (!trio_keys(i + 1))
                    problem(blk_file, step_line[i + 1], "the installed guard step sets more than run, name and working-directory")
                if (!trio_keys(i))
                    problem(blk_file, at, "a yarn install step sets more than run, name and working-directory")
            }
        }

        function finish_file() {
            if (cur_file == "") return
            end_steps()
            if (!top_perm) problem(cur_file, 1, "no top-level permissions: block holding contents: read")
            else if (!top_read) problem(cur_file, top_perm, "the top-level permissions: block does not hold contents: read")
        }

        # step_key <text> <line> — a key line of step n.
        function step_key(t, at,    k, v) {
            k = t
            sub(/:.*$/, "", k)
            if (k !~ /^[A-Za-z_][A-Za-z0-9_-]*$/ || index(t, ":") == 0)
                problem(FILENAME, at, "a step tools-check cannot read (not one key per line)")
            step_keys[n] = step_keys[n] " " k
            if (k != "run") return
            v = trim(substr(t, 5))
            has_run[n]++
            run_step = n
            in_run = 1
            run_ind = key_ind
            if (v ~ /^[|>][-+0-9]*([ \t]+#.*)?$/) {
                run_lit = (substr(v, 1, 1) == "|")
                run_buf = ""
                return
            }
            run_lit = 0
            if (substr(v, 1, 1) == q || substr(v, 1, 1) == "\"") v = substr(v, 2, length(v) - 2)
            else v = bare_value(v)
            run_buf = v
        }

        function text_checks(line) {
            if (line ~ /pull_request_target/) problem(FILENAME, FNR, "pull_request_target")
            if (line ~ /secrets[ \t]*(\.|\[)/) problem(FILENAME, FNR, "a secrets expression")
            if (line ~ /^[ \t]*(-[ \t]+)?secrets[ \t]*:/) problem(FILENAME, FNR, "a secrets: key")
        }

        BEGIN {
            GUARD = "node frontend/install-check.mjs"
            ni = split("install add remove upgrade upgrade-interactive global import link", a, " ")
            for (i = 1; i <= ni; i++) INSTALLS[a[i]] = 1
            nv = split("--cwd --modules-folder --cache-folder --global-folder --link-folder --preferred-cache-folder --use-yarnrc --mutex --network-concurrency --network-timeout --registry --otp --proxy --https-proxy", a, " ")
            for (i = 1; i <= nv; i++) VALUE_FLAGS[a[i]] = 1
        }

        FNR == 1 {
            finish_file()
            cur_file = FILENAME
            seen[FILENAME] = 1
            top_perm = 0; top_read = 0; in_top_perm = 0
            in_steps = 0; in_run = 0; n = 0
            prev_line = ""; prev_ind = 0
        }

        {
            sub(/\r$/, "")
            line = $0
            ind = indent(line)
            blank = (line ~ /^[ \t]*$/)
            comment = (line ~ /^[ \t]*#/)

            if (in_run) {
                if (blank || ind > run_ind) {
                    text_checks(line)
                    run_buf = run_buf (run_lit ? "\n" : " ") trim(line)
                    next
                }
                close_run()
            }
            if (blank || comment) next
            text_checks(line)

            if (ind == 0) {
                end_steps()
                in_top_perm = 0
                if (line ~ /^permissions[ \t]*:/) {
                    top_perm = FNR
                    v = line
                    sub(/^permissions[ \t]*:/, "", v)
                    if (bare_value(v) == "") in_top_perm = 1
                }
            } else if (in_top_perm) {
                k = trim(line)
                v = k
                sub(/[ \t]*:.*$/, "", k)
                sub(/^[^:]*:/, "", v)
                v = bare_value(v)
                if (k == "contents" && v == "read") top_read = 1
                if (v != "read" && v != "none") problem(FILENAME, FNR, "a top-level permission other than read or none: " trim(line))
            } else {
                if (line ~ /^[ \t]*(-[ \t]+)?permissions[ \t]*:/) problem(FILENAME, FNR, "a permissions: below the top level")
                if (in_steps && ind <= steps_ind) end_steps()
                if (!in_steps && line ~ /^ *steps[ \t]*:[ \t]*(#.*)?$/) {
                    in_steps = 1; steps_ind = ind; item_ind = -1; n = 0; blk_file = FILENAME
                } else if (in_steps && ind > steps_ind && (item_ind < 0 || ind == item_ind) && line ~ /^ *-([ \t]|$)/) {
                    item_ind = ind
                    n++
                    has_run[n] = 0; run_text[n] = ""; step_keys[n] = ""; step_line[n] = FNR
                    if (match(line, /^ *-[ \t]+/) && substr(line, RLENGTH + 1) !~ /^#/) {
                        key_ind = RLENGTH
                        step_key(substr(line, RLENGTH + 1), FNR)
                    } else {
                        key_ind = -1
                    }
                } else if (in_steps && n > 0 && ind > item_ind && (key_ind < 0 || ind == key_ind)) {
                    key_ind = ind
                    step_key(substr(line, ind + 1), FNR)
                } else if (line ~ /(^|[ \t{,-])run[ \t]*:/ &&
                           !(line ~ /^ *run[ \t]*:[ \t]*(#.*)?$/ && prev_line ~ /^ *defaults[ \t]*:[ \t]*(#.*)?$/ && ind > prev_ind)) {
                    problem(FILENAME, FNR, "a run: that is not a step command tools-check can read")
                }
            }
            prev_line = line
            prev_ind = ind
        }

        END {
            finish_file()
            for (i = 1; i < ARGC; i++)
                if (!(ARGV[i] in seen)) problem(ARGV[i], 0, "an empty workflow file")
        }
    ' $workflow_files
}

safety="$(ci_safety)"
if [ -n "$safety" ]; then
    while IFS= read -r problem; do
        pin_error "$problem"
    done <<EOF
$safety
EOF
fi

if [ "$pins_only" = 0 ]; then
    # local_version <label> <want> <command…> — warn when the tool is missing or differs.
    local_version() {
        label="$1"
        want="$2"
        shift 2
        if ! have="$("$@" 2>/dev/null)"; then
            echo "tools-check: warning: $label is not installed (CI uses $want; run just setup)"
            return
        fi
        have="$(printf '%s\n' "$have" | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n 1)"
        if [ "$have" = "$want" ]; then
            echo "tools-check: $label $have matches CI"
        else
            echo "tools-check: warning: $label is $have here, CI uses $want (run just setup)"
        fi
    }
    local_version typos "$TYPOS_VERSION" typos --version
    local_version cargo-deny "$CARGO_DENY_VERSION" cargo deny --version
    local_version buf "$BUF_VERSION" buf --version
    local_version just "$JUST_VERSION" just --version
    local_version nodes "${NODES_VERSION#v}" nodes --version
    channel="$(awk -F'"' '/^channel[[:space:]]*=/ { print $2; exit }' rust-toolchain.toml)"
    local_version rustc "$channel" rustc --version
fi

if [ "$errors" -ne 0 ]; then
    exit 1
fi
echo "tools-check: workflow pins agree with the Justfile"
