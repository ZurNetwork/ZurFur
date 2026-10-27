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
# is not pinned to a full commit SHA or is not at the one commit allowed_actions
# names for it, a workflow breaks a CI safety rule, or
# frontend/install-check.mjs holds no readable INSTALL_COMMAND. A local version
# that differs from CI's prints a warning and leaves the exit code alone.
#
# The CI safety rules: every checkout sets persist-credentials: false; every
# actions/upload-artifact step sets path frontend/ui/shots/output/**/*.png and
# retention-days 7, and runs only on exactly
# `if: failure() && steps.shots.outcome == 'failure'`; and the rules in the
# comment above ci_safety (permissions, secrets, installs, package managers,
# containers, actions and the YAML shapes this reader can follow).
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

# The only actions a step may use, one `<action>@<SHA>` per line, each at the
# one commit reviewed for it: GitHub fetches any SHA under an action's name,
# even one that lives only in a fork. A new action or a bump is a deliberate
# two-line change, after review: the workflow's `uses:` line and its line here.
allowed_actions="
actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1
actions/setup-node@949feb2413d6458794dcd2491c4babbbce0c15c1
Swatinem/rust-cache@6323deb102c322ba6fcbdcafc7e3dddab59af2b6
bufbuild/buf-action@85aebf73123b5c15fd5528aaecbf9129cddf7fa7
EmbarkStudios/cargo-deny-action@3c6349835b2b7b196a839186cb8b78e02f7b5f25
crate-ci/typos@7f9d0819ca1c008ade73f4b41fa9d258d48f605b
extractions/setup-just@53165ef7e734c5c07cb06b3c8e7b647c5aa16db3
actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9
"

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
            sub(/\r$/, "")
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

# check_step_input <action> <input> <value> [<why>] — at least one step uses
# <action>, and every such step sets <input> to <value>; <why> names the rule
# in the message (default: Justfile pin).
check_step_input() {
    counts="$(step_pins "$1" "$2" "$3")"
    steps="${counts% *}"
    pinned="${counts#* }"
    if [ "$steps" -eq 0 ]; then
        pin_error "no workflow uses $1"
    elif [ "$steps" != "$pinned" ]; then
        pin_error "$steps $1 step(s) but $pinned set $2: '$3' (${4:-Justfile pin})"
    fi
}
check_step_input bufbuild/buf-action version "$BUF_VERSION"
check_step_input extractions/setup-just just-version "$JUST_VERSION"
# No checkout leaves its token in .git/config for later steps to read.
check_step_input actions/checkout persist-credentials false "CI safety rule"
# The one upload is the screenshot run's PNG files, kept 7 days: a wider path or a
# longer retention fails.
check_step_input actions/upload-artifact path 'frontend/ui/shots/output/**/*.png' "CI safety rule"
check_step_input actions/upload-artifact retention-days 7 "CI safety rule"

# step_if_pins <action> <condition> — prints "<steps> <pinned>": how many
# steps use <action>, and how many of those carry exactly one step-level
# `if:`, whose value is exactly <condition>. A step's keys are the lines at
# its first key's indent, before or after its `uses:`; an `if:` under `with:`
# or `env:` never counts.
step_if_pins() {
    # shellcheck disable=SC2086 # $workflow_files is a newline list without spaces
    awk -v action="$1" -v want="$2" -v q="'" '
        function indent(s) { match(s, /^ */); return RLENGTH }
        function close_item() {
            if (in_item && uses_action) { steps++; if (ifs == 1 && if_ok) ok++ }
            in_item = 0
        }
        function step_key(t,    v) {
            if (match(t, /^uses:[ \t]*/)) {
                v = substr(t, RLENGTH + 1)
                sub("^[\"" q "]", "", v)
                if (index(v, action "@") == 1) uses_action = 1
            } else if (match(t, /^if:[ \t]*/)) {
                v = substr(t, RLENGTH + 1)
                sub(/[ \t]+#.*$/, "", v)
                sub(/[ \t]+$/, "", v)
                ifs++
                if (v == want) if_ok = 1
            }
        }
        FNR == 1 { close_item() }
        {
            sub(/\r$/, "")
            line = $0
            if (line ~ /^ *(#.*)?$/) next
            if (in_item && indent(line) > dash_ind) {
                if (indent(line) == key_ind) step_key(substr(line, key_ind + 1))
                next
            }
            close_item()
            if (match(line, /^ *- +/)) {
                key_ind = RLENGTH
                dash_ind = indent(line)
                in_item = 1
                uses_action = 0
                ifs = 0
                if_ok = 0
                step_key(substr(line, key_ind + 1))
            }
        }
        END { close_item(); print steps + 0, ok + 0 }
    ' $workflow_files
}

# check_step_if <action> <condition> — at least one step uses <action>, and
# every such step runs only on exactly <condition>.
check_step_if() {
    counts="$(step_if_pins "$1" "$2")"
    steps="${counts% *}"
    pinned="${counts#* }"
    if [ "$steps" -eq 0 ]; then
        pin_error "no workflow uses $1"
    elif [ "$steps" != "$pinned" ]; then
        pin_error "$steps $1 step(s) but $pinned set if: '$2' (CI safety rule)"
    fi
}
# The upload runs only when the screenshot step (id: shots) itself failed, not
# on any earlier step's failure.
check_step_if actions/upload-artifact "failure() && steps.shots.outcome == 'failure'"

# The guarded install's exact text: the INSTALL_COMMAND the install guard
# exports, from its one `export const INSTALL_COMMAND = '<command>';` line.
install_guard="frontend/install-check.mjs"
install_command="$(awk -v q="'" '
    BEGIN { head = "export const INSTALL_COMMAND = " q }
    { sub(/\r$/, "") }
    index($0, head) == 1 && substr($0, length($0) - 1) == q ";" {
        n++
        v = substr($0, length(head) + 1)
        v = substr(v, 1, length(v) - 2)
    }
    END { if (n == 1 && v ~ /[^ ]/ && index(v, q) == 0 && index(v, "\\") == 0) print v }
' "$install_guard" 2>/dev/null || true)"
if [ -z "$install_command" ]; then
    pin_error "$install_guard: no INSTALL_COMMAND line tools-check can read (one line: export const INSTALL_COMMAND = '<command>';)"
fi

# ci_safety — prints one "<file>:<line>: <problem>" per break of the CI safety
# rules, in every workflow file:
#   - a top-level `permissions:` block holding `contents: read`, every entry
#     read or none; no `permissions:` below the top level (a job's);
#   - no `pull_request_target`, no `secrets.`/`secrets[` expression and no
#     `secrets:` key, outside YAML comments (a run script's comments count);
#   - every yarn install (bare `yarn`, or a yarn command whose subcommand is
#     install, add, remove, upgrade, upgrade-interactive, global, import,
#     link or unplug, whatever flags come first) is a step whose `run:` is,
#     whitespace squeezed, exactly the install guard's INSTALL_COMMAND; it
#     carries --frozen-lockfile and --ignore-scripts, stands alone in its
#     step's `run:`, and sits between a step running exactly
#     `node frontend/install-check.mjs` and one running exactly
#     `node frontend/install-check.mjs --installed`; those three steps set
#     nothing but run, name and working-directory;
#   - no command anywhere in a `run:` or a `shell:` is npm, npx, pnpm, pnpx,
#     bun, bunx or corepack (bare, by path or quoted, wherever it sits in the
#     command); no yarn command there takes --use-yarnrc or has the
#     subcommand create, policies, set, init, config, workspace, self-update
#     or dlx (each fetches and runs a package, changes which yarn runs, or
#     runs any yarn command);
#   - no `container:` key and no `services:` key anywhere (a job's, a
#     step's or deeper), and no word docker or podman, in any letter case,
#     bare, by path or quoted, in a `run:` or a `shell:` outside its shell
#     comments: a workflow's own text starts no container (the screenshot
#     script and the test suites start theirs);
#   - every `run:` is a step's (a `defaults: run:` block aside), so a shape
#     this reader can't follow fails rather than hiding an install;
#   - every `uses:` is a step's, so a job-level `uses:` (a reusable workflow
#     call) fails, and names, before its `@`, exactly one of allowed_actions
#     (no other action, sub-path, local `./` action or `docker://` image),
#     and, after it, exactly the SHA allowed_actions holds for that action;
#   - no mapping key is quoted (`'uses':`, `"uses":`, block or flow style)
#     and no line holds an explicit key (`? uses`, its value on a `:` line):
#     YAML reads each as the bare key, so a `uses:` or `run:` spelled that
#     way would slip past the rules above; nor is a quoted scalar left open
#     at its line's end, nor does a tag (`!!str uses:`), an anchor (`&a uses:`)
#     or an alias (`*a :`) sit where a key starts (a sequence item's `- &a`
#     included). Script text in a `run:` or a `shell:` is exempt; a quoted
#     value (`key: 'x'`, `- 'x'`), and a tag, anchor or alias on a value
#     (`key: &a x`, `key: *a`, `key: !!str x`), are fine;
#   - no line, script text included, holds a line break this reader does not
#     split on, which YAML could read as a new line it never sees: a CR that
#     does not end a CRLF, a NEL (U+0085), an LS (U+2028) or a PS (U+2029).
# Top level means column 0: GitHub's workflow keys sit there, so any
# `permissions:` indented under them is a job's (or deeper).
ci_safety() {
    # The line breaks awk does not split on, matched as bytes in the C locale
    # so every awk reads them alike.
    # shellcheck disable=SC2086 # $workflow_files is a newline list without spaces
    LC_ALL=C awk '
        function found(what) { print FILENAME ":" FNR ": " what ", a line break tools-check cannot read" }
        {
            s = $0
            sub(/\r$/, "", s)
            if (index(s, "\r")) found("a lone CR (not a CRLF line end)")
            if (index(s, "\302\205")) found("a NEL (U+0085)")
            if (index(s, "\342\200\250")) found("an LS (U+2028)")
            if (index(s, "\342\200\251")) found("a PS (U+2029)")
        }
    ' $workflow_files
    # shellcheck disable=SC2086 # $workflow_files is a newline list without spaces
    TOOLS_CHECK_INSTALL="$install_command" TOOLS_CHECK_ACTIONS="$allowed_actions" awk -v q="'" '
        function indent(s) { match(s, /^ */); return RLENGTH }
        function trim(s) { gsub(/^[ \t]+|[ \t]+$/, "", s); return s }
        function squeeze(s) { gsub(/[ \t\n]+/, " ", s); return trim(s) }
        function bare_value(s) { sub(/[ \t]+#.*$/, "", s); return trim(s) }
        function problem(file, at, msg) { print file ":" at ": " msg }
        function unquote(w) { gsub("\\$[\"" q "]", "", w); gsub("[\"" q "\\\\]", "", w); return w }
        function base(w) { w = unquote(w); sub(/^.*\//, "", w); return w }
        function is_yarn(w) { w = base(w); return w == "yarn" || w == "yarnpkg" }
        function add_bad(msg) { if (!(msg in bad_seen)) { bad_seen[msg] = 1; bad[++n_bad] = msg } }

        # check_action <file> <line> <value> — the action a `uses:` value
        # names, its text before `@`, is exactly one on the allow list, and
        # its ref, the text after that `@`, is exactly the SHA listed for it.
        function check_action(file, at, v,    name, ref) {
            name = trim(v)
            sub("^[\"" q "]", "", name)
            ref = name
            sub("[]@ \t,}\"" q "#].*$", "", name)
            if (!(name in ACTIONS)) {
                problem(file, at, "an action not on the allow list: " (name == "" ? "(none)" : name))
                return
            }
            ref = substr(ref, length(name) + 1)
            if (substr(ref, 1, 1) == "@") ref = substr(ref, 2)
            else ref = ""
            sub("[] \t,}\"" q "#].*$", "", ref)
            if (ref != ACTIONS[name])
                problem(file, at, "an action not at its reviewed commit: " name "@" (ref == "" ? "(no ref)" : ref) " (allowed_actions lists the one reviewed SHA)")
        }

        # quoted_end <text> <i> — where the quoted scalar opening at <i> closes
        # on this line (a doubled single quote, or a backslash in double
        # quotes, escapes), or 0 when it runs past the line.
        function quoted_end(s, i,    c, open, j, len) {
            open = substr(s, i, 1)
            len = length(s)
            for (j = i + 1; j <= len; j++) {
                c = substr(s, j, 1)
                if (open == "\"" && c == "\\") { j++; continue }
                if (c != open) continue
                if (open == q && substr(s, j + 1, 1) == q) { j++; continue }
                return j
            }
            return 0
        }

        # key_shape <line> — "" when every mapping key on <line> is plain;
        # otherwise the shape it found: a quoted key (a quoted scalar then
        # `:`) or an explicit key (`?`), each at a key position (the line
        # start, after a `- `, or after a flow `{`, `[` or `,`), outside
        # quotes, comments and `${{ … }}` expressions in plain text; a tag
        # (`!`), an anchor (`&`) or an alias (`*`) at a key position, since
        # each can stand before or for a key; or a quoted scalar left open at
        # the line end, since the next line could then hide a quoted key from
        # this reader.
        function key_shape(s,    i, c, len, at_key, prev, j, flow) {
            len = length(s)
            at_key = 1
            prev = " "
            flow = 0
            for (i = 1; i <= len; i++) {
                c = substr(s, i, 1)
                if (c == " " || c == "\t") { prev = c; continue }
                # A `${{ … }}` expression outside a flow collection is plain
                # text to YAML, its braces, quotes and `!` included.
                if (c == "$" && flow == 0 && substr(s, i + 1, 2) == "{{") {
                    j = index(substr(s, i + 3), "}}")
                    if (j == 0) return ""
                    i += j + 3
                    at_key = 0
                    prev = "}"
                    continue
                }
                if (at_key && (c == "-" || c == "?") && (i == len || substr(s, i + 1, 1) ~ /[ \t]/)) {
                    if (c == "?") return "an explicit key (?)"
                    prev = c
                    continue
                }
                if (at_key && c == "!") return "a tag (!) at a key position"
                if (at_key && c == "&") return "an anchor (&) at a key position"
                if (at_key && c == "*") return "an alias (*) at a key position"
                if ((c == q || c == "\"") && prev ~ /[ \t{[,:]/) {
                    j = quoted_end(s, i)
                    if (j == 0) return "a quoted scalar that runs past its line"
                    if (at_key && substr(s, j + 1) ~ /^[ \t]*:/) return "a quoted mapping key"
                    i = j
                    at_key = 0
                    prev = c
                    continue
                }
                if (c == "#" && prev ~ /[ \t]/) return ""
                if (c == "{" || c == "[") flow++
                if ((c == "}" || c == "]") && flow > 0) flow--
                at_key = (c == "{" || c == "[" || c == ",")
                prev = c
            }
            return ""
        }

        # not_a_step_use <line> — a `uses:` that is not a step key fails, and
        # what it names is checked too.
        function not_a_step_use(line) {
            problem(FILENAME, FNR, "a uses: that is not a step key (a reusable workflow call, or a shape tools-check cannot read)")
            if (match(line, /uses[ \t]*:/)) check_action(FILENAME, FNR, substr(line, RSTART + RLENGTH))
        }

        # strip_comments <text> — <text> without its shell comments: a `#`
        # that starts a word outside quotes, to the end of its line.
        function strip_comments(s,    out, i, c, sq, dq, prev, skip) {
            out = ""; sq = 0; dq = 0; prev = " "; skip = 0
            for (i = 1; i <= length(s); i++) {
                c = substr(s, i, 1)
                if (skip) {
                    if (c != "\n") continue
                    skip = 0
                } else if (sq) {
                    if (c == q) sq = 0
                } else if (c == "\\") {
                    out = out c
                    i++
                    c = substr(s, i, 1)
                } else if (dq) {
                    if (c == "\"") dq = 0
                } else if (c == q) {
                    sq = 1
                } else if (c == "\"") {
                    dq = 1
                } else if (c == "#" && prev ~ /[ \t\n;&|()]/) {
                    skip = 1
                    continue
                }
                out = out c
                prev = c
            }
            return out
        }

        # yarn_scan <text> — sets n_cmds (shell commands in <text>), n_inst
        # (those holding a yarn install), inst_frozen/inst_ignore (whether
        # every install carries the flag), and bad[1..n_bad] (every other
        # installer, refused yarn subcommand, --use-yarnrc, docker or podman,
        # once each).
        function yarn_scan(text,    lines, nl, i, ln, cmds, nc, j, c, w, nw, k, m, a, n_sub, bound, is_inst, f, g, rt) {
            n_cmds = 0; n_inst = 0; inst_frozen = 1; inst_ignore = 1
            n_bad = 0; split("", bad_seen)
            # A container runtime by name, its line continuations joined and
            # its quotes and backslashes gone, as the shell would read the word.
            rt = text
            gsub(/\\\n/, "", rt)
            rt = tolower(unquote(strip_comments(rt)))
            if (rt ~ /(^|[^a-z0-9_])docker([^a-z0-9_]|$)/) add_bad("docker in a run: or shell: (scripts/gallery-shots.sh alone names the container runtime)")
            if (rt ~ /(^|[^a-z0-9_])podman([^a-z0-9_]|$)/) add_bad("podman in a run: or shell: (scripts/gallery-shots.sh alone names the container runtime)")
            gsub(/\\\n/, " ", text)
            text = strip_comments(text)
            nl = split(text, lines, "\n")
            for (i = 1; i <= nl; i++) {
                ln = lines[i]
                gsub(/&&|\|\||[;|&()`]/, "\n", ln)
                nc = split(ln, cmds, "\n")
                for (j = 1; j <= nc; j++) {
                    c = trim(cmds[j])
                    if (c == "") continue
                    n_cmds++
                    nw = split(c, w, /[ \t<>]+/)
                    is_inst = 0
                    for (k = 1; k <= nw; k++) {
                        if (base(w[k]) in INSTALLERS) add_bad("a package manager other than yarn: " base(w[k]))
                        if (!is_yarn(w[k])) continue
                        # yarn takes its first word no flag binds as the
                        # subcommand; a word after a flag may be that flag value
                        # or the subcommand, so each one counts.
                        n_sub = 0
                        bound = 0
                        for (m = k + 1; m <= nw; m++) {
                            a = unquote(w[m])
                            if (a ~ /^--use-yarnrc(=|$)/) add_bad("a yarn --use-yarnrc, which can swap the yarn that runs")
                            if (a ~ /^-/) { bound = 1; continue }
                            if (n_sub < 0) continue
                            n_sub++
                            if (a in YARN_REFUSED) add_bad("a refused yarn subcommand: " a)
                            if (a in INSTALLS) is_inst = 1
                            if (!bound) n_sub = -1
                            bound = 0
                        }
                        if (n_sub == 0) is_inst = 1
                    }
                    if (!is_inst) continue
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

        function close_shell(    b) {
            if (!in_shell) return
            in_shell = 0
            yarn_scan(shell_buf)
            for (b = 1; b <= n_bad; b++) problem(shell_file, shell_line, bad[b])
            if (n_inst) problem(shell_file, shell_line, "a yarn install in a shell:")
        }

        function end_steps(    i, at, b) {
            close_run()
            if (!in_steps) return
            in_steps = 0
            for (i = 1; i <= n; i++) {
                if (!has_run[i]) continue
                yarn_scan(run_text[i])
                at = step_line[i]
                for (b = 1; b <= n_bad; b++) problem(blk_file, at, bad[b])
                if (n_inst == 0) continue
                if (INSTALL == "") problem(blk_file, at, "a yarn install, and no INSTALL_COMMAND to match it against")
                else if (squeeze(run_text[i]) != INSTALL) problem(blk_file, at, "a yarn install that is not exactly `" INSTALL "`")
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
            if (k == "uses") check_action(FILENAME, at, substr(t, index(t, ":") + 1))
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
            INSTALL = ENVIRON["TOOLS_CHECK_INSTALL"]
            ni = split("install add remove upgrade upgrade-interactive upgradeInteractive global import link unplug", a, " ")
            for (i = 1; i <= ni; i++) INSTALLS[a[i]] = 1
            ni = split("npm npx pnpm pnpx bun bunx corepack", a, " ")
            for (i = 1; i <= ni; i++) INSTALLERS[a[i]] = 1
            ni = split("create policies set init config workspace self-update dlx", a, " ")
            for (i = 1; i <= ni; i++) YARN_REFUSED[a[i]] = 1
            ni = split(ENVIRON["TOOLS_CHECK_ACTIONS"], a, " ")
            for (i = 1; i <= ni; i++) {
                at = index(a[i], "@")
                if (at > 1) ACTIONS[substr(a[i], 1, at - 1)] = substr(a[i], at + 1)
            }
        }

        FNR == 1 {
            close_shell()
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
            if (in_shell) {
                if (blank || ind > shell_ind) {
                    text_checks(line)
                    shell_buf = shell_buf (shell_lit ? "\n" : " ") trim(line)
                    next
                }
                close_shell()
            }
            if (blank || comment) next
            text_checks(line)
            # A run: or shell: value is script text (the installer rules read
            # it), so only the keys of every other line are checked for their shape.
            if (line !~ /^[ \t]*(-[ \t]+)?(run|shell)[ \t]*:/) {
                shape = key_shape(line)
                if (shape != "") problem(FILENAME, FNR, shape ", a shape tools-check cannot read")
                if (line ~ /(^|[ \t{,])container[ \t]*:/) problem(FILENAME, FNR, "a container: key (no job or step asks for a container image)")
                if (line ~ /(^|[ \t{,])services[ \t]*:/) problem(FILENAME, FNR, "a services: key (no job starts service containers)")
            }
            # A shell: (in a step or under defaults) runs its command too; its
            # value, continuation lines included, is scanned like a run:.
            if (match(line, /^[ \t]*(-[ \t]+)?shell[ \t]*:/)) {
                in_shell = 1
                shell_file = FILENAME
                shell_line = FNR
                shell_ind = indent(line)
                if (match(line, /^ *- +/)) shell_ind = RLENGTH
                v = line
                sub(/^[ \t]*(-[ \t]+)?shell[ \t]*:/, "", v)
                v = trim(v)
                shell_lit = (substr(v, 1, 1) == "|")
                shell_buf = (v ~ /^[|>][-+0-9]*([ \t]+#.*)?$/) ? "" : v
            }

            if (ind == 0) {
                end_steps()
                in_top_perm = 0
                if (line ~ /(^|[ \t{,-])uses[ \t]*:/) not_a_step_use(line)
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
                } else if (line ~ /(^|[ \t{,-])uses[ \t]*:/) {
                    not_a_step_use(line)
                }
            }
            prev_line = line
            prev_ind = ind
        }

        END {
            close_shell()
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
