#!/bin/sh
# SubagentStart / SessionStart(compact|resume) hook: re-state the project
# pins (.claude/pins.md) — a subagent starts with none of the main
# session's context, and compaction summarizes it away, so the standing
# facts about how this repo is worked are re-injected at exactly those
# two seams. Silent for every other event (including SessionStart
# startup/clear/fork). POSIX sh + jq.
#
# Same defensive posture as the other hooks here: any internal failure
# degrades to silence, never to a nonzero exit — the `trap` below is the
# backstop, forcing exit 0 no matter what happens above it.
set -eu
trap 'exit 0' EXIT

script_dir="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=scripts/hooks/lib.sh
. "$script_dir/lib.sh"

jq_bin="jq"
stat_bin="stat"
if [ "${ZURFUR_HOOKS_TEST:-}" = "1" ]; then
    jq_bin="${ZURFUR_JQ_BIN:-jq}"
    stat_bin="${ZURFUR_STAT_BIN:-stat}"
fi

project_dir="${CLAUDE_PROJECT_DIR:-}"
if [ -z "$project_dir" ]; then
    project_dir="$(cd "$script_dir/../.." && pwd)"
fi

MAX_PINS=40
MAX_CHARS=8000
HEADER="Project pins (from .claude/pins.md; standing facts about how this repo is worked):"

hooks_are_off "pins" "$project_dir" && exit 0

command -v "$jq_bin" >/dev/null 2>&1 || exit 0

input="$(cat)"

event="$(printf '%s' "$input" | "$jq_bin" -r '.hook_event_name // ""' 2>/dev/null || true)"

case "$event" in
    SubagentStart) ;;
    SessionStart)
        source_val="$(printf '%s' "$input" | "$jq_bin" -r '.source // ""' 2>/dev/null || true)"
        case "$source_val" in
            compact | resume) ;;
            *) exit 0 ;;
        esac
        ;;
    *) exit 0 ;;
esac

pins_file="$project_dir/.claude/pins.md"
[ -f "$pins_file" ] || exit 0

# Bullet lines only — a heading, a comment, or a blank line never counts as
# a pin. Untrimmed; each still needs neutralizing below.
all_pins="$(awk '/^- / { print }' "$pins_file" 2>/dev/null || true)"

# The untracked, user-level pins file — machine-specific facts (the
# sed->sd aliasing note, for example) that don't belong in the repo.
# Appended only when it exists, isn't a symlink, and is PROVABLY owned by
# the current user; its pins count toward both limits below, same as the
# tracked ones. Fails CLOSED: if `stat` itself fails (missing, denied,
# any other error), that is "ownership not proven", not "ownership
# assumed" — the file is skipped, never used on the strength of an
# unreadable check.
local_pins_file="${ZURFUR_PINS_LOCAL:-$HOME/.claude/pins.local.md}"
if [ -f "$local_pins_file" ] && [ ! -L "$local_pins_file" ]; then
    local_owner=""
    local_owner_known=0
    if local_owner="$("$stat_bin" -c '%u' "$local_pins_file" 2>/dev/null)"; then
        local_owner_known=1
    elif local_owner="$("$stat_bin" -f '%u' "$local_pins_file" 2>/dev/null)"; then
        local_owner_known=1
    fi
    if [ "$local_owner_known" -eq 1 ] && [ "$local_owner" = "$(id -u)" ]; then
        local_pins="$(awk '/^- / { print }' "$local_pins_file" 2>/dev/null || true)"
        if [ -n "$local_pins" ]; then
            all_pins="$all_pins
$local_pins"
        fi
    fi
fi

[ -n "$all_pins" ] || exit 0

total_count="$(printf '%s\n' "$all_pins" | grep -c '^- ' || true)"

# Limit 1: at most MAX_PINS lines, oldest-first (file order).
count_capped="$(printf '%s\n' "$all_pins" | grep '^- ' | head -n "$MAX_PINS")"
shown_count="$(printf '%s\n' "$count_capped" | grep -c '^- ' || true)"
count_dropped=$((total_count - shown_count))

# Neutralize every line that survived the count cap — pins are tracked,
# reviewed text, but the local file isn't, and this is still text landing
# in a model's context, so it gets the same treatment as any other
# injected data.
neutralized=""
while IFS= read -r line; do
    [ -n "$line" ] || continue
    nline="$(neutralize "$line")"
    neutralized="${neutralized:+$neutralized
}$nline"
done <<EOF
$count_capped
EOF

# Limit 2: at most MAX_CHARS total. Order-preserving PREFIX: stop at the
# first pin that doesn't fit and drop it plus everything after it, never
# skip one oversized pin and keep checking shorter ones further down —
# that would silently reorder which pins survive. At most 40 of them, so
# a per-line `wc -c` here is cheap, nothing like the thousands-of-refs
# case the refs hook has to avoid forking per candidate for. Reserves
# room for the eventual "N more" hint using a safe upper bound on its
# digit count.
hint_reserve="$(printf '\n  %s more in .claude/pins.md' "$total_count" | wc -c)"
budget=$((MAX_CHARS - hint_reserve))
header_len="$(printf '%s' "$HEADER" | wc -c)"
kept=""
kept_len="$header_len"
kept_count=0
while IFS= read -r pline; do
    [ -n "$pline" ] || continue
    pline_len="$(printf '%s' "$pline" | wc -c)"
    candidate_len=$((kept_len + pline_len + 1))
    if [ "$candidate_len" -gt "$budget" ]; then
        break
    fi
    kept="${kept:+$kept
}$pline"
    kept_len="$candidate_len"
    kept_count=$((kept_count + 1))
done <<EOF
$neutralized
EOF
byte_dropped=$((shown_count - kept_count))

remaining=$((count_dropped + byte_dropped))

body="$HEADER"
if [ -n "$kept" ]; then
    body="$body
$kept"
fi
if [ "$remaining" -gt 0 ]; then
    body="$body
  $remaining more in .claude/pins.md"
fi

# Final hard backstop, same shape as the other hooks: whatever the above
# computed, the whole block is bounded to MAX_CHARS bytes, with the
# marker's own length reserved exactly and the cut backed off to a valid
# UTF-8 boundary (utf8_safe_head) rather than a bare byte offset.
body_len="$(printf '%s' "$body" | wc -c)"
if [ "$body_len" -gt "$MAX_CHARS" ]; then
    truncation_marker="
  ... truncated at $MAX_CHARS chars"
    marker_len="$(printf '%s' "$truncation_marker" | wc -c)"
    keep_len=$((MAX_CHARS - marker_len))
    [ "$keep_len" -lt 0 ] && keep_len=0
    body="$(printf '%s' "$body" | utf8_safe_head "$keep_len")$truncation_marker"
fi

set +e
output="$("$jq_bin" -n --arg event "$event" --arg ctx "$body" '{hookSpecificOutput: {hookEventName: $event, additionalContext: $ctx}}' 2>/dev/null)"
jq_rc=$?
set -e

if [ "$jq_rc" -eq 0 ] && [ -n "$output" ]; then
    printf '%s\n' "$output"
fi
