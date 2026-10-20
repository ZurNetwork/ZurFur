#!/bin/sh
# SessionStart(startup|resume|clear|compact) hook: states that the main
# session is the Orchestrator and which skill CLAUDE.md's first rule has it
# load, so a session whose first message is a task still gets the fact.
# SessionStart never fires for a subagent, so a Spawn never sees it. Silent
# for every other event and source. POSIX sh; jq only to read the event,
# and without jq the settings.json matcher alone decides.
#
# The text is a fixed statement of fact, never an imperative: command-shaped
# text in injected context can trip prompt-injection defenses. Any internal
# failure degrades to silence; the `trap` forces exit 0.
set -eu
trap 'exit 0' EXIT

script_dir="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=scripts/hooks/lib.sh
. "$script_dir/lib.sh"

jq_bin="jq"
if [ "${ZURFUR_HOOKS_TEST:-}" = "1" ]; then
    jq_bin="${ZURFUR_JQ_BIN:-jq}"
fi

project_dir="${CLAUDE_PROJECT_DIR:-}"
if [ -z "$project_dir" ]; then
    project_dir="$(cd "$script_dir/../.." && pwd)"
fi

REMINDER="Zurfur runs the teams way of working. This is the main session, so it is the Orchestrator: CLAUDE.md's first rule has it load the \`teams:orchestrate\` skill (Skill tool) before its first reply, and again after compaction."

hooks_are_off "orchestrate" "$project_dir" && exit 0

input="$(cat)"

if command -v "$jq_bin" >/dev/null 2>&1; then
    event="$(printf '%s' "$input" | "$jq_bin" -r '.hook_event_name // ""' 2>/dev/null || true)"
    source_val="$(printf '%s' "$input" | "$jq_bin" -r '.source // ""' 2>/dev/null || true)"
    case "$event" in
        SessionStart | "") ;;
        *) exit 0 ;;
    esac
    case "$source_val" in
        startup | resume | clear | compact | "") ;;
        *) exit 0 ;;
    esac
fi

printf '%s\n' "$REMINDER"
