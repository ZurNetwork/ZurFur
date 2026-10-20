#!/bin/sh
# Compares this machine's tools with the versions CI uses, and checks that the
# workflows agree with those pins. Run via `just tools-check`, which exports
# TYPOS_VERSION, CARGO_DENY_VERSION, BUF_VERSION, JUST_VERSION and
# NODES_VERSION from the Justfile.
#
#   tools-check.sh              both checks; local drift only warns
#   tools-check.sh --pins-only  the workflow check alone (what CI runs)
#
# Exits 1 only when the workflows disagree with the Justfile's pins or an
# action is not pinned to a full commit SHA. A local version that differs from
# CI's prints a warning and leaves the exit code alone.
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
