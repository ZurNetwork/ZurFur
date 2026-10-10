#!/bin/sh
# Compares this machine's tools with the versions CI uses, and checks that the
# workflows agree with those pins. Run via `just tools-check`, which exports
# TYPOS_VERSION, CARGO_DENY_VERSION and BUF_VERSION from the Justfile.
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
errors=0

pin_error() {
    echo "tools-check: $1" >&2
    errors=$((errors + 1))
}

# Every `uses:` names a full 40-hex commit SHA followed by `# v<version>`.
unpinned="$(grep -hE '^[[:space:]]*-?[[:space:]]*uses:' "$workflows"/*.yml | grep -vE '@[0-9a-f]{40} # v[0-9]' || true)"
if [ -n "$unpinned" ]; then
    pin_error "actions not pinned to a commit SHA with a '# v<version>' comment:"
    printf '  %s\n' "$unpinned" >&2
fi

# check_every_use <action> <required text> — each line using <action> carries <text>.
check_every_use() {
    lines="$(grep -hF "$1@" "$workflows"/*.yml || true)"
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
if ! grep -qF "version: '$BUF_VERSION'" "$workflows/ci.yml"; then
    pin_error "ci.yml's buf version is not '$BUF_VERSION' (Justfile pin)"
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
    channel="$(awk -F'"' '/^channel[[:space:]]*=/ { print $2; exit }' rust-toolchain.toml)"
    local_version rustc "$channel" rustc --version
fi

if [ "$errors" -ne 0 ]; then
    exit 1
fi
echo "tools-check: workflow pins agree with the Justfile"
