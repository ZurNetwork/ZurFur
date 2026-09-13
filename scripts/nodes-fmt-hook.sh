#!/bin/sh
# Claude Code PostToolUse hook (Write|Edit), wired in .claude/settings.json:
# keep every NODE.json canonical without anyone remembering to. Reads the tool
# call from stdin and, when the written file is a NODE.json, runs `nodes fmt`
# on it. A schema error (the model wrote a NODE.json the tool refuses) is fed
# back to the model via exit 2 + stderr, so it fixes the file at once.
#
# Fails SOFT: a clone without the `nodes` binary prints a one-line hint and
# exits 0 — the hard backstop is `just nodes-check` in the gate and CI.
set -eu

input="$(cat)"
if command -v jq >/dev/null 2>&1; then
    file="$(printf '%s' "$input" | jq -r '.tool_input.file_path // empty')"
else
    file="$(printf '%s' "$input" | grep -o '"file_path"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed -E 's/^"file_path"[[:space:]]*:[[:space:]]*"(.*)"$/\1/')"
fi

case "$file" in
    */NODE.json|NODE.json) ;;
    *) exit 0 ;;
esac

if ! command -v nodes >/dev/null 2>&1; then
    echo "nodes not installed — run \`just setup\` (or \`just nodes-install\`); $file left unformatted"
    exit 0
fi

if ! output="$(nodes fmt "$file" 2>&1)"; then
    printf '%s\n' "$output" >&2
    exit 2
fi
[ -z "$output" ] || printf '%s\n' "$output"
