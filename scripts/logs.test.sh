#!/bin/sh
# Tests for scripts/logs.sh, behind `just logs-tail` and `just logs-clean`: an
# unknown name lists the known ones and fails, a known one is followed, and
# clean stops the followers before it removes logs/, and only a root carrying
# the wrapper's marker. A stub `tail` and a stub `docker` stand in. Run directly, or via `just scripts-test`. Needs bash.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/logs.sh"
work="$(mktemp -d)"
logs="$work/logs"
mkdir -p "$work/bin" "$logs/latest" "$logs/dev-back" "$logs/compose/db"
: >"$logs/.zurfur-logs"
failures=0

# The stub tail prints its arguments, one per line.
cat >"$work/bin/tail" <<'SHIM'
#!/bin/sh
printf '%s\n' "$@"
SHIM
cat >"$work/bin/docker" <<'SHIM'
#!/bin/sh
trap 'exit 0' TERM
while :; do sleep 0.2; done
SHIM
chmod +x "$work/bin/tail" "$work/bin/docker"
: >"$logs/dev-back/20261014T120000Z.log"
: >"$logs/compose/db/20261014T120000Z.log"
ln -s ../dev-back/20261014T120000Z.log "$logs/latest/dev-back.log"
ln -s ../compose/db/20261014T120000Z.log "$logs/latest/db.log"

logs_sh() {
    PATH="$work/bin:$PATH" ZURFUR_LOG_DIR="$logs" bash "$under_test" "$@"
}
cleanup() {
    logs_sh clean >/dev/null 2>&1 || true
    rm -rf "$work"
}
trap cleanup EXIT

ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1: $2"
    failures=$((failures + 1))
}

# --- tail -----------------------------------------------------------------------
status=0
listed="$(logs_sh tail nope 2>&1)" || status=$?
if [ "$status" = "1" ] && printf '%s\n' "$listed" | grep -qx '  db' &&
    printf '%s\n' "$listed" | grep -qx '  dev-back'; then
    ok "an unknown name lists the names it knows and exits 1"
else
    fail "an unknown name lists the names it knows and exits 1" "status $status: $listed"
fi
followed="$(logs_sh tail dev-back)"
if [ "$followed" = "$(printf '%s\n' -F "$logs/latest/dev-back.log")" ]; then
    ok "a name follows that one latest file"
else
    fail "a name follows that one latest file" "$followed"
fi
followed="$(logs_sh tail)"
if [ "$followed" = "$(printf '%s\n' -F "$logs/latest/db.log" "$logs/latest/dev-back.log")" ]; then
    ok "no name follows every latest file"
else
    fail "no name follows every latest file" "$followed"
fi

# --- clean ----------------------------------------------------------------------
stranger="$work/someone-elses"
mkdir -p "$stranger"
: >"$stranger/precious.txt"
status=0
refused="$(PATH="$work/bin:$PATH" ZURFUR_LOG_DIR="$stranger" bash "$under_test" clean 2>&1)" || status=$?
if [ "$status" = "1" ] && [ -f "$stranger/precious.txt" ] &&
    printf '%s\n' "$refused" | grep -q "not removing it"; then
    ok "clean refuses a root without the logs marker and leaves it whole"
else
    fail "clean refuses a root without the logs marker and leaves it whole" "status $status: $refused"
fi
PATH="$work/bin:$PATH" ZURFUR_LOG_DIR="$logs" bash "$script_dir/compose-logs.sh" start 2026-10-14T12:00:00Z db
follower="$(awk '{ print $1 }' "$logs/compose/db/follower.pid")"
said="$(logs_sh clean)"
tries=0
while kill -0 "$follower" 2>/dev/null && [ "$tries" -lt 50 ]; do
    sleep 0.1
    tries=$((tries + 1))
done
if [ ! -e "$logs" ] && ! kill -0 "$follower" 2>/dev/null &&
    printf '%s\n' "$said" | grep -q "removed $logs"; then
    ok "clean stops the followers, removes logs/ and says so"
else
    fail "clean stops the followers, removes logs/ and says so" "$said"
fi
said="$(logs_sh clean)"
if printf '%s\n' "$said" | grep -q "nothing to remove"; then
    ok "clean with no logs/ says there is nothing to remove"
else
    fail "clean with no logs/ says there is nothing to remove" "$said"
fi

if [ "$failures" -gt 0 ]; then
    echo "$failures logs test(s) failed"
    exit 1
fi
echo "all logs tests pass"
