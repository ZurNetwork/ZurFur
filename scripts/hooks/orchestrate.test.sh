#!/bin/sh
# Tests for scripts/hooks/orchestrate.sh (the SessionStart Orchestrator
# reminder), plus checks on its wiring in the tracked .claude/settings.json.
# Run directly, or via `just hooks-test`. POSIX sh + jq + awk only — no `sed`.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
repo_root="$(cd "$script_dir/.." && cd .. && pwd)"
under_test="$script_dir/orchestrate.sh"
settings="$repo_root/.claude/settings.json"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

failures=0
ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1"
    failures=$((failures + 1))
}

expected="Zurfur runs the teams way of working. This is the main session, so it is the Orchestrator: CLAUDE.md's first rule has it load the \`teams:orchestrate\` skill (Skill tool) before its first reply, and again after compaction."

proj="$work/proj"
mkdir -p "$proj/.claude"

hookjson() {
    # hookjson <event> [source]
    if [ -n "${2:-}" ]; then
        jq -n --arg ev "$1" --arg src "$2" '{session_id: "s1", hook_event_name: $ev, source: $src}'
    else
        jq -n --arg ev "$1" '{session_id: "s1", hook_event_name: $ev, agent_id: "a1", agent_type: "Explore"}'
    fi
}

# run_with <stdin file> [env assignments...] — runs the hook under a scrubbed
# environment (env -i: an ambient ZURFUR_HOOKS_OFF or CLAUDE_PROJECT_DIR from
# a live session never leaks in), checks it exited 0.
run_with() {
    stdin_file="$1"
    shift
    set +e
    env -i PATH="$PATH" HOME="$work" CLAUDE_PROJECT_DIR="$proj" "$@" \
        sh "$under_test" <"$stdin_file" >"$work/out" 2>"$work/err"
    rc=$?
    set -e
    [ "$rc" -eq 0 ] || fail "exit code $rc (stderr: $(cat "$work/err"))"
}

# run <event> [source] [env assignments...]
run() {
    hookjson "$1" "${2:-}" >"$work/in"
    shift
    [ "$#" -gt 0 ] && shift
    run_with "$work/in" "$@"
}

expect_reminder() {
    name="$1"
    got="$(cat "$work/out")"
    if [ "$got" = "$expected" ]; then
        ok "$name"
    else
        fail "$name: expected the reminder, got: [$got]"
    fi
}

expect_silent() {
    name="$1"
    if [ -s "$work/out" ]; then
        fail "$name: expected silence, got: $(cat "$work/out")"
    else
        ok "$name (silent)"
    fi
}

# --- every SessionStart source the matcher names prints the reminder ---
for src in startup resume clear compact; do
    run "SessionStart" "$src"
    expect_reminder "SessionStart $src"
done

# --- any other source, and any other event, is silent ---
run "SessionStart" "fork"
expect_silent "SessionStart fork"
run "SubagentStart" ""
expect_silent "SubagentStart"
run "PreToolUse" ""
expect_silent "unrelated event (PreToolUse)"

# --- no event on stdin: the settings.json matcher alone decides ---
: >"$work/empty"
run_with "$work/empty"
expect_reminder "empty stdin"

# --- no jq: the event can't be read, so the matcher alone decides ---
hookjson "SessionStart" "fork" >"$work/in"
run_with "$work/in" ZURFUR_HOOKS_TEST=1 ZURFUR_JQ_BIN="$work/no-such-jq"
expect_reminder "jq missing: prints regardless of source"

# --- the kill switch ---
run "SessionStart" "startup" ZURFUR_HOOKS_OFF=orchestrate
expect_silent "ZURFUR_HOOKS_OFF=orchestrate"
run "SessionStart" "startup" ZURFUR_HOOKS_OFF=all
expect_silent "ZURFUR_HOOKS_OFF=all"
run "SessionStart" "startup" ZURFUR_HOOKS_OFF="pins,refs"
expect_reminder "ZURFUR_HOOKS_OFF=pins,refs (unrelated) still fires"
touch "$proj/.claude/hooks.off"
run "SessionStart" "startup"
expect_silent ".claude/hooks.off present"
rm -f "$proj/.claude/hooks.off"

# --- CLAUDE_PROJECT_DIR unset: falls back to the script's own repo ---
fallback="$work/fallback"
mkdir -p "$fallback/scripts/hooks" "$fallback/.claude"
cp "$under_test" "$script_dir/lib.sh" "$fallback/scripts/hooks/"
touch "$fallback/.claude/hooks.off"
hookjson "SessionStart" "startup" >"$work/in"
set +e
env -i PATH="$PATH" HOME="$work" sh "$fallback/scripts/hooks/orchestrate.sh" <"$work/in" >"$work/out" 2>"$work/err"
rc=$?
set -e
[ "$rc" -eq 0 ] || fail "fallback: exit code $rc"
expect_silent "CLAUDE_PROJECT_DIR fallback reads the script's own repo's hooks.off"

# --- the text itself: one line, no tag-shaped text, no imperatives ---
run "SessionStart" "startup"
lines="$(awk 'END { print NR }' "$work/out")"
[ "$lines" -eq 1 ] && ok "text: one line" || fail "text: $lines lines"
if grep -q '[<>]' "$work/out"; then fail "text: contains '<' or '>'"; else ok "text: no tag-shaped text"; fi
if grep -qiE '\b(must|never|always|important|do not|don.t|ignore)\b' "$work/out"; then
    fail "text: imperative wording: $(cat "$work/out")"
else
    ok "text: no imperative wording"
fi
grep -qF '`teams:orchestrate`' "$work/out" && ok "text: names the skill" || fail "text: skill name missing"

# --- the wiring in .claude/settings.json ---
matcher_cmds="$(jq -r '.hooks.SessionStart[] | select(.matcher == "startup|resume|clear|compact") | .hooks[].command' "$settings")"
if printf '%s\n' "$matcher_cmds" | grep -qF 'scripts/hooks/orchestrate.sh'; then
    ok "settings: SessionStart startup|resume|clear|compact runs orchestrate.sh"
else
    fail "settings: no SessionStart entry with matcher startup|resume|clear|compact runs orchestrate.sh"
fi
wired_count="$(jq -r '[.hooks[][] | .hooks[].command | select(contains("scripts/hooks/orchestrate.sh"))] | length' "$settings")"
[ "$wired_count" -eq 1 ] && ok "settings: orchestrate.sh wired exactly once" || fail "settings: orchestrate.sh wired $wired_count times"

# Every script a hook command names exists, so a deleted hook can't stay wired.
jq -r '.hooks[][] | .hooks[].command' "$settings" \
    | awk '{ for (i = 1; i <= NF; i++) if ($i ~ /\$CLAUDE_PROJECT_DIR/) { p = $i; gsub(/"/, "", p); sub(/^\$CLAUDE_PROJECT_DIR\/?/, "", p); print p } }' \
    >"$work/wired"
missing=0
while IFS= read -r path; do
    [ -n "$path" ] || continue
    if [ ! -f "$repo_root/$path" ]; then
        fail "settings: hook command names a missing script: $path"
        missing=1
    fi
done <"$work/wired"
[ -s "$work/wired" ] || fail "settings: found no hook script paths to check"
[ "$missing" -eq 0 ] && ok "settings: every wired hook script exists"

if [ "$failures" -eq 0 ]; then
    echo "orchestrate.test.sh: all cases passed"
else
    echo "orchestrate.test.sh: $failures case(s) failed"
    exit 1
fi
