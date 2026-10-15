#!/bin/sh
# Tests for scripts/compose-logs.sh against a stub `docker`: one detached
# follower per service with its pidfile and log file, a new start stopping the
# old follower (and never a process that isn't ours), retention, and stop.
# Run directly, or via `just scripts-test`. Needs bash, setsid and ps.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/compose-logs.sh"
work="$(mktemp -d)"
logs="$work/logs"
mkdir -p "$work/bin" "$work/calls"
failures=0

# The stub records each `docker` call's arguments, prints a line naming the
# service, then follows forever in a child process, as podman-compose runs
# `podman logs -f` under it; the child's pid lands in <service>.<pid>.child.
cat >"$work/bin/docker" <<'SHIM'
#!/bin/sh
for service in "$@"; do :; done
printf '%s\n' "$@" >"$STUB_CALLS/$service.$$"
echo "stub log line for $service"
sh -c 'trap "exit 0" TERM; while :; do sleep 0.2; done' &
echo "$!" >"$STUB_CALLS/$service.$$.child"
wait
SHIM
chmod +x "$work/bin/docker"

compose_logs() {
    PATH="$work/bin:$PATH" ZURFUR_LOG_DIR="$logs" STUB_CALLS="$work/calls" \
        bash "$under_test" "$@"
}
bystander=""
cleanup() {
    compose_logs stop >/dev/null 2>&1 || true
    [ -n "$bystander" ] && kill "$bystander" 2>/dev/null
    rm -rf "$work"
}
trap cleanup EXIT

ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1: $2"
    failures=$((failures + 1))
}
alive() { kill -0 "$1" 2>/dev/null; }
pid_of() { awk 'NR == 1 { print $1 }' "$logs/compose/$1/follower.pid" 2>/dev/null; }
# child_of <follower pid> <service>: the stub's following child.
child_of() {
    eventually [ -s "$work/calls/$2.$1.child" ] && cat "$work/calls/$2.$1.child"
}
gone() { ! kill -0 "$1" 2>/dev/null; }
# eventually <command…>: retries for up to 5 s.
eventually() {
    tries=0
    until "$@"; do
        tries=$((tries + 1))
        [ "$tries" -ge 50 ] && return 1
        sleep 0.1
    done
}

# --- one follower per service, with its pidfile --------------------------------
mkdir -p "$logs/compose/db"
: >"$logs/.zurfur-logs"
for day in 01 02 03 04 05 06 07 08 09 10 11 12; do
    : >"$logs/compose/db/202610${day}T000000Z.log"
done
compose_logs start 2026-10-14T12:00:00Z db pds
db_pid="$(pid_of db)"
pds_pid="$(pid_of pds)"
if [ -n "$db_pid" ] && [ -n "$pds_pid" ] && alive "$db_pid" && alive "$pds_pid"; then
    ok "start leaves a running follower and a pidfile per service"
else
    fail "start leaves a running follower and a pidfile per service" "db '$db_pid', pds '$pds_pid'"
fi
leads_its_group() { [ "$(ps -o pgid= -p "$1" | tr -d ' ')" = "$1" ]; }
if eventually leads_its_group "$db_pid"; then
    ok "each follower leads its own process group (setsid)"
else
    fail "each follower leads its own process group (setsid)" "$(ps -o pid,pgid,sid -p "$db_pid")"
fi
expected_argv="$(printf '%s\n' compose logs -f --no-color -t --since 2026-10-14T12:00:00Z db)"
if eventually [ -s "$work/calls/db.$db_pid" ] && [ "$(cat "$work/calls/db.$db_pid")" = "$expected_argv" ]; then
    ok "the follower runs docker compose logs -f --no-color -t --since <since> <service>"
else
    fail "the follower runs docker compose logs -f --no-color -t --since <since> <service>" \
        "$(cat "$work/calls/db.$db_pid" 2>&1)"
fi
link="$(readlink "$logs/latest/db.log" 2>/dev/null || true)"
case "$link" in
    ../compose/db/*.log) ok "latest/<service>.log links into compose/<service>/" ;;
    *) fail "latest/<service>.log links into compose/<service>/" "it points at '$link'" ;;
esac
if eventually grep -q "stub log line for db" "$logs/latest/db.log"; then
    ok "the follower's output lands in the file"
else
    fail "the follower's output lands in the file" "$(cat "$logs/latest/db.log" 2>&1)"
fi
kept="$(find "$logs/compose/db" -name '*.log' | wc -l | tr -d ' ')"
if [ "$kept" = "10" ] && [ ! -e "$logs/compose/db/20261001T000000Z.log" ]; then
    ok "the newest 10 files are kept per service"
else
    fail "the newest 10 files are kept per service" "$(ls "$logs/compose/db" | tr '\n' ' ')"
fi

# --- a new start stops the old follower first ------------------------------------
old_db_child="$(child_of "$db_pid" db)"
compose_logs start 2026-10-14T12:05:00Z db
new_db_pid="$(pid_of db)"
if eventually gone "$db_pid" && alive "$new_db_pid" &&
    [ "$new_db_pid" != "$db_pid" ] && alive "$pds_pid"; then
    ok "a new start stops that service's old follower, and only that one"
else
    fail "a new start stops that service's old follower, and only that one" \
        "old $db_pid, new $new_db_pid, pds $pds_pid"
fi
if [ -n "$old_db_child" ] && eventually gone "$old_db_child"; then
    ok "a new start stops the old follower's whole process group, its child too"
else
    fail "a new start stops the old follower's whole process group, its child too" \
        "child '$old_db_child' still runs"
fi

# --- never a process that isn't ours ------------------------------------------
sleep 30 &
bystander=$!
printf '%s %s\n' "$bystander" "not-its-start-time" >"$logs/compose/pds/follower.pid"
compose_logs start 2026-10-14T12:10:00Z pds
if alive "$bystander" && [ "$(pid_of pds)" != "$bystander" ]; then
    ok "a pidfile naming someone else's process never kills it"
else
    fail "a pidfile naming someone else's process never kills it" "bystander $bystander"
fi
# The pds follower from the first start is orphaned by the forged pidfile.
kill -TERM -- "-$pds_pid" 2>/dev/null || true

# --- stop ends every follower --------------------------------------------------
db_pid="$(pid_of db)"
pds_pid="$(pid_of pds)"
db_child="$(child_of "$db_pid" db)"
pds_child="$(child_of "$pds_pid" pds)"
compose_logs stop
if [ -n "$db_child" ] && [ -n "$pds_child" ] && eventually gone "$db_child" &&
    eventually gone "$pds_child"; then
    ok "stop ends each follower's whole process group, its child too"
else
    fail "stop ends each follower's whole process group, its child too" \
        "children '$db_child' '$pds_child'"
fi
if eventually sh -c "! kill -0 $db_pid 2>/dev/null && ! kill -0 $pds_pid 2>/dev/null" &&
    [ ! -e "$logs/compose/db/follower.pid" ] && [ ! -e "$logs/compose/pds/follower.pid" ]; then
    ok "stop ends every follower and removes its pidfile"
else
    fail "stop ends every follower and removes its pidfile" "db $db_pid, pds $pds_pid"
fi

# --- a populated root without the marker ---------------------------------------
stranger="$work/someone-elses"
mkdir -p "$stranger/notes"
printf 'keep me\n' >"$stranger/precious.txt"
listing() { find "$1" -exec stat -c '%n %a %s' {} + | sort; find "$1" -type f -exec cksum {} + | sort; }
before="$(listing "$stranger")"
calls_before="$(ls "$work/calls" | wc -l)"
status=0
PATH="$work/bin:$PATH" ZURFUR_LOG_DIR="$stranger" STUB_CALLS="$work/calls" \
    bash "$under_test" start 2026-10-14T12:20:00Z db 2>"$work/stderr" || status=$?
if [ "$status" != "0" ] && [ "$before" = "$(listing "$stranger")" ] &&
    [ "$calls_before" = "$(ls "$work/calls" | wc -l)" ] && grep -q "refusing" "$work/stderr"; then
    ok "start refuses a populated root without the marker, follows nothing and writes nothing"
else
    fail "start refuses a populated root without the marker, follows nothing and writes nothing" \
        "status $status, stderr: $(cat "$work/stderr")"
fi

if [ "$failures" -gt 0 ]; then
    echo "$failures compose-logs test(s) failed"
    exit 1
fi
echo "all compose-logs tests pass"
