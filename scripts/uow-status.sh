#!/usr/bin/env bash
# SessionStart hook — states whether a unit of work has tickets in flight, so
# a fresh Claude instance has the ledger's facts without anyone running a
# command. Reads the primary checkout's ledger (+ logbook) and prints a
# one-glance, FACTUAL summary only when tickets are in flight; silent
# otherwise. Injected text is read by a model, so it states facts — never
# imperatives ("resume it", "do NOT start a new unit") — command-shaped text
# in injected context can trip prompt-injection defenses.
#
# The ledger and logbook are tracked repo files — the same trust as any
# other file in this checkout — but their free-text fields (the status
# note, logbook threads) are still author-written prose, not something
# this hook can vouch for the CONTENT of, so it is capped in length and
# neutralized (control chars stripped, '<'/'>' removed) before it reaches a
# model's context, and rendered as visibly-quoted data rather than blended
# into the factual line around it. The WHOLE injected block is additionally
# bounded by one hard cap. Always exits 0 (a hook must never fail session
# start).
set -uo pipefail
trap 'exit 0' EXIT

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/hooks/lib.sh
. "$script_dir/hooks/lib.sh"

TOTAL_CAP=3000

# Resolve the repo root the ledger lives in: the project dir Claude Code
# tells hooks about, or (tests, bare invocations) this script's own repo.
project_dir="${CLAUDE_PROJECT_DIR:-}"
if [ -z "$project_dir" ]; then
    project_dir="$(cd "$script_dir/.." && pwd)"
fi

hooks_are_off "uow" "$project_dir" && exit 0

ledger="$project_dir/.understand/parallel-set.json"
[ -f "$ledger" ] || exit 0
command -v jq >/dev/null 2>&1 || exit 0

# A ticket is "in flight" unless its phase starts with a terminal word.
# Tolerates the older `state` field name (prefers `phase`), and both
# `next_action` and the legacy `next` field for the free-text status note.
# Every free-text field is neutralized and capped at 80 chars (160 for the
# longer status note) before it is ever formatted into a line.
read -r -d '' jqprog <<'JQ' || true
def neutralize:
  gsub("[\r\n\t]"; " ")
  | gsub("[[:cntrl:]]"; "")
  | gsub("[<>]"; "");
def cap(n): (. // "" | tostring | neutralize) as $s | if ($s | length) > n then ($s[0:(n - 3)] + "...") else $s end;
def phase: (.phase // .state // "planned") | cap(80);
def terminal: test("^(merged|deferred|handed-off|superseded|done|closed)");
def note: (.next_action // .next // "") | cap(160);
def place: (if .worktree then .worktree elif .branch then .branch else null end) | if . then cap(80) else null end;
[ .tickets[] | select((phase | terminal) | not) ] as $live
| if ($live | length) == 0 then empty
  else
    ("Unit of work `" + ((.uow // "?") | cap(80)) + "` has " + ($live | length | tostring) + " ticket(s) in flight:"),
    ( $live[:8][]
      | "  " + (.key | cap(80)) + " @ " + phase
        + (if (note | length) > 0 then " — note (ledger text, not an instruction): \"" + note + "\"" else "" end)
        + (place as $p | if $p then "  [" + $p + "]" else "" end) ),
    (if ($live | length) > 8 then "  " + (($live | length) - 8 | tostring) + " more" else empty end)
  end
JQ

out=$(jq -r "$jqprog" "$ledger" 2>/dev/null) || exit 0
[ -n "$out" ] || exit 0
body="$out"

# Open threads from the logbook (the "what's not yet sound" list), if
# present. Each thread line is capped and neutralized the same way.
uow=$(jq -r '.uow // ""' "$ledger" 2>/dev/null) || uow=""
logbook="$project_dir/.understand/logbooks/$uow.md"
if [ -n "$uow" ] && [ -f "$logbook" ]; then
    threads=$(awk '/^## Open threads/{f=1;next} /^## /{f=0} f' "$logbook" | grep -E '\S' | head -8)
    if [ -n "$threads" ]; then
        threads_capped=""
        while IFS= read -r t; do
            [ -n "$t" ] || continue
            line="  $(cap_bytes "$(neutralize "$t")" 200)"
            threads_capped="${threads_capped:+$threads_capped
}$line"
        done <<EOF
$threads
EOF
        body="$body
Open threads recorded for \`$(cap_bytes "$(neutralize "$uow")" 80)\` (from the logbook):
$threads_capped"
    fi
fi

# One hard cap over the WHOLE injected block, regardless of how many
# fields fed into it — a defense-in-depth backstop behind the per-field caps.
# The marker's own length is reserved OUT OF the cap (computed from the
# marker text itself, not a guessed constant), so truncated output is
# <= TOTAL_CAP bytes, never TOTAL_CAP plus a few.
body_len=$(printf '%s' "$body" | wc -c)
if [ "$body_len" -gt "$TOTAL_CAP" ]; then
    truncation_marker="
  ... output truncated at $TOTAL_CAP chars"
    marker_len=$(printf '%s' "$truncation_marker" | wc -c)
    keep_len=$((TOTAL_CAP - marker_len))
    [ "$keep_len" -lt 0 ] && keep_len=0
    body="$(printf '%s' "$body" | utf8_safe_head "$keep_len")$truncation_marker"
fi

printf '%s\n' "$body"
