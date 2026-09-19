#!/bin/sh
# Test double for the `nodes` binary, used by design-refs.test.sh via
# ZURFUR_NODES_BIN. `chain <path> --root <dir> --json` ignores the path and
# prints whatever fixture $NODES_STUB_CHAIN_FILE points at (or `[]`) — the
# tests drive behavior through which fixture is loaded, not through path
# matching, since design-refs.sh's own logic never inspects chain content
# beyond what this stub controls.
set -eu
case "${1:-}" in
    chain)
        if [ -n "${NODES_STUB_CHAIN_FILE:-}" ] && [ -f "$NODES_STUB_CHAIN_FILE" ]; then
            cat "$NODES_STUB_CHAIN_FILE"
        else
            echo '[]'
        fi
        ;;
    *)
        exit 1
        ;;
esac
