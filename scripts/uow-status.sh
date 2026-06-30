#!/usr/bin/env bash
# SessionStart hook — surface an in-flight unit of work so a fresh Claude instance
# resumes WITHOUT a command. Reads the primary checkout's ledger (+ logbook) and
# prints a one-glance summary only when a unit is in flight; quiet when idle.
# Always exits 0 (a hook must never fail the session start).
set -uo pipefail

primary=$(git worktree list --porcelain 2>/dev/null | awk '/^worktree /{print $2; exit}')
[ -n "${primary:-}" ] || exit 0
ledger="$primary/.understand/parallel-set.json"
[ -f "$ledger" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0

# A ticket is "in flight" unless its phase is terminal (merged/deferred/handed-off).
# Tolerates the older `state` field name (prefers `phase`).
read -r -d '' jqprog <<'JQ' || true
def phase: (.phase // .state // "planned");
[ .tickets[] | phase as $p | select((["merged","deferred","handed-off"] | index($p)) | not) ] as $live
| if ($live | length) == 0 then empty
  else
    ("⏳ In-flight unit of work `" + (.uow // "?") + "` — resume it; do NOT start a new unit:"),
    ( $live[]
      | "  • " + .key + " @ " + phase
        + (if .next_action then " — next: " + .next_action else "" end)
        + (if .worktree then "  [" + .worktree + "]"
           elif .branch then "  [" + .branch + "]" else "" end) )
  end
JQ

out=$(jq -r "$jqprog" "$ledger" 2>/dev/null) || exit 0
[ -n "$out" ] || exit 0
printf '%s\n' "$out"

# Open threads from the logbook (the "what's not yet sound" list), if present.
uow=$(jq -r '.uow // ""' "$ledger" 2>/dev/null)
logbook="$primary/.understand/logbooks/$uow.md"
if [ -n "$uow" ] && [ -f "$logbook" ]; then
  threads=$(awk '/^## Open threads/{f=1;next} /^## /{f=0} f' "$logbook" | grep -E '\S' | head -8)
  if [ -n "$threads" ]; then
    echo "  Open threads (from logbook):"
    printf '%s\n' "$threads" | sed 's/^/    /'
  fi
fi
echo "  → Read the ledger + logbook, then continue each ticket from its phase (see /unit-of-work → Resume)."
