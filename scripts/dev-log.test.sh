#!/bin/sh
# Tests for scripts/dev-log.sh, the wrapper every dev recipe runs through:
# the file and its latest link, retention, plain text on the file side, the
# terminal's bytes untouched, the exit status, a stamp collision and Ctrl-C.
# A stub `date` pins the stamp. Run directly, or via `just scripts-test`.
# Needs bash, perl and python3.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/dev-log.sh"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT
failures=0

# A `date` that prints $STUB_STAMP for the wrapper's stamp and defers to the
# real one otherwise.
mkdir -p "$work/bin"
real_date="$(command -v date)"
cat >"$work/bin/date" <<SHIM
#!/bin/sh
if [ "\${1:-}" = "-u" ] && [ "\${2:-}" = "+%Y%m%dT%H%M%SZ" ] && [ -n "\${STUB_STAMP:-}" ]; then
    echo "\$STUB_STAMP"
    exit 0
fi
exec "$real_date" "\$@"
SHIM
chmod +x "$work/bin/date"

# ok <name> / fail <name> <why>
ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1: $2"
    failures=$((failures + 1))
}

# run_wrapper <logs> <stamp> <name> <command…>: the wrapper with its log root at
# <logs> and the stamp pinned; stdout goes to $work/stdout.
run_wrapper() {
    logs="$1" stamp="$2"
    shift 2
    PATH="$work/bin:$PATH" ZURFUR_LOG_DIR="$logs" STUB_STAMP="$stamp" \
        bash "$under_test" "$@" >"$work/stdout"
}

# --- the file and its latest link -------------------------------------------
logs="$work/logs-file"
run_wrapper "$logs" 20261014T120000Z demo printf 'hello\n'
if [ -f "$logs/demo/20261014T120000Z.log" ]; then
    ok "the run writes logs/<name>/<stamp>.log"
else
    fail "the run writes logs/<name>/<stamp>.log" "$(ls -R "$logs" 2>&1)"
fi
link="$(readlink "$logs/latest/demo.log" 2>/dev/null || true)"
if [ "$link" = "../demo/20261014T120000Z.log" ]; then
    ok "latest/<name>.log is a relative link to it"
else
    fail "latest/<name>.log is a relative link to it" "it points at '$link'"
fi
if [ "$(cat "$logs/latest/demo.log")" = "hello" ]; then
    ok "the file holds the command's output"
else
    fail "the file holds the command's output" "$(cat "$logs/latest/demo.log")"
fi
mode_of() { stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"; }
if [ "$(mode_of "$logs/demo/20261014T120000Z.log")" = "600" ] &&
    [ "$(mode_of "$logs/demo")" = "700" ] && [ "$(mode_of "$logs/latest")" = "700" ]; then
    ok "the file is 0600 and its directories 0700"
else
    fail "the file is 0600 and its directories 0700" \
        "$(mode_of "$logs/demo/20261014T120000Z.log") $(mode_of "$logs/demo") $(mode_of "$logs/latest")"
fi

# --- retention ----------------------------------------------------------------
logs="$work/logs-retention"
mkdir -p "$logs/demo"
for day in 01 02 03 04 05 06 07 08 09 10 11 12; do
    : >"$logs/demo/202610${day}T000000Z.log"
done
run_wrapper "$logs" 20261013T000000Z demo true
kept="$(ls "$logs/demo" | wc -l | tr -d ' ')"
if [ "$kept" = "10" ] && [ -f "$logs/demo/20261013T000000Z.log" ] &&
    [ ! -e "$logs/demo/20261001T000000Z.log" ] && [ ! -e "$logs/demo/20261003T000000Z.log" ] &&
    [ -f "$logs/demo/20261004T000000Z.log" ]; then
    ok "the newest 10 files are kept"
else
    fail "the newest 10 files are kept" "$(ls "$logs/demo" | tr '\n' ' ')"
fi

# --- plain text in the file, the terminal's bytes untouched --------------------
logs="$work/logs-plain"
colored='\033[1;31mred\033[0m\r\nbar \033]8;;http://x\007link\033]8;;\007\033[2K\rdone\n'
run_wrapper "$logs" 20261014T120000Z demo printf "$colored"
printf "$colored" >"$work/expected"
if cmp -s "$work/stdout" "$work/expected"; then
    ok "stdout carries the command's bytes unchanged"
else
    fail "stdout carries the command's bytes unchanged" "$(od -c "$work/stdout" | head -5)"
fi
printf 'red\nbar linkdone\n' >"$work/expected-plain"
if cmp -s "$logs/latest/demo.log" "$work/expected-plain"; then
    ok "the file has no escapes and no carriage returns"
else
    fail "the file has no escapes and no carriage returns" "$(od -c "$logs/latest/demo.log" | head -5)"
fi

# --- stderr is logged too -----------------------------------------------------
logs="$work/logs-stderr"
run_wrapper "$logs" 20261014T120000Z demo sh -c 'echo out; echo err >&2'
if grep -qx err "$logs/latest/demo.log" && grep -qx err "$work/stdout"; then
    ok "stderr reaches the file and the terminal"
else
    fail "stderr reaches the file and the terminal" "$(cat "$logs/latest/demo.log")"
fi

# --- the exit status ----------------------------------------------------------
logs="$work/logs-status"
status=0
run_wrapper "$logs" 20261014T120000Z demo sh -c 'echo before; exit 7' || status=$?
if [ "$status" = "7" ] && grep -qx before "$logs/latest/demo.log"; then
    ok "the wrapper exits with the command's status"
else
    fail "the wrapper exits with the command's status" "status $status"
fi

# --- a name collision ---------------------------------------------------------
logs="$work/logs-collision"
run_wrapper "$logs" 20261014T120000Z demo echo first
run_wrapper "$logs" 20261014T120000Z demo echo second
run_wrapper "$logs" 20261014T120000Z demo echo third
if [ "$(cat "$logs/demo/20261014T120000Z.log")" = "first" ] &&
    [ "$(cat "$logs/demo/20261014T120000Z_2.log")" = "second" ] &&
    [ "$(cat "$logs/demo/20261014T120000Z_3.log")" = "third" ] &&
    [ "$(readlink "$logs/latest/demo.log")" = "../demo/20261014T120000Z_3.log" ]; then
    ok "a taken stamp gets a suffix, never an overwrite"
else
    fail "a taken stamp gets a suffix, never an overwrite" "$(ls "$logs/demo" | tr '\n' ' ')"
fi

# --- a name that isn't one ----------------------------------------------------
logs="$work/logs-name"
status=0
run_wrapper "$logs" 20261014T120000Z ../escape true 2>/dev/null || status=$?
if [ "$status" != "0" ] && [ ! -e "$work/escape" ]; then
    ok "a name with a slash is refused"
else
    fail "a name with a slash is refused" "status $status"
fi

# --- a latest link pointing at a directory -----------------------------------------
logs="$work/logs-link"
mkdir -p "$logs/latest" "$work/elsewhere"
ln -s "$work/elsewhere" "$logs/latest/demo.log"
run_wrapper "$logs" 20261014T120000Z demo echo linked
if [ "$(readlink "$logs/latest/demo.log")" = "../demo/20261014T120000Z.log" ] &&
    [ -z "$(ls -A "$work/elsewhere")" ]; then
    ok "a latest link pointing at a directory is replaced, never written through"
else
    fail "a latest link pointing at a directory is replaced, never written through" \
        "link '$(readlink "$logs/latest/demo.log")', elsewhere: $(ls -A "$work/elsewhere")"
fi
if [ -f "$logs/.zurfur-logs" ]; then
    ok "the root carries the marker logs-clean looks for"
else
    fail "the root carries the marker logs-clean looks for" "$(ls -A "$logs")"
fi

# --- Ctrl-C --------------------------------------------------------------------
# interrupt <logs> <script>: runs the wrapper over `sh <script>` in its own
# session, as a terminal's foreground job, waits for the script's `started`,
# sends SIGINT to the whole group as Ctrl-C does, and prints how the wrapper
# ended: its exit status, or -2 when it died of SIGINT.
interrupt() {
    PATH="$work/bin:$PATH" ZURFUR_LOG_DIR="$1" STUB_STAMP=20261014T120000Z \
        python3 - "$under_test" "$2" "$work/stdout" <<'PY'
import os, signal, subprocess, sys, time
wrapper, script, stdout = sys.argv[1:]
with open(stdout, "wb") as out:
    run = subprocess.Popen(["bash", wrapper, "demo", "sh", script], stdout=out,
                           stderr=subprocess.STDOUT, start_new_session=True)
    for _ in range(100):
        with open(stdout, "rb") as seen:
            if b"started" in seen.read():
                break
        time.sleep(0.05)
    os.killpg(run.pid, signal.SIGINT)
    print(run.wait(timeout=20))
PY
}

cat >"$work/last-words.sh" <<'CMD'
trap 'echo last words; exit 130' INT
echo started
while :; do sleep 0.1; done
CMD
logs="$work/logs-last-words"
ended="$(interrupt "$logs" "$work/last-words.sh")"
if grep -qx "last words" "$logs/latest/demo.log" 2>/dev/null && [ "$ended" = "-2" ]; then
    ok "Ctrl-C: the file keeps the command's last line, and the wrapper dies of SIGINT"
else
    fail "Ctrl-C: the file keeps the command's last line, and the wrapper dies of SIGINT" \
        "ended $ended, file: $(cat "$logs/latest/demo.log" 2>&1)"
fi

cat >"$work/no-trap.sh" <<'CMD'
echo started
while :; do sleep 0.1; done
CMD
logs="$work/logs-no-trap"
ended="$(interrupt "$logs" "$work/no-trap.sh")"
if grep -qx started "$logs/latest/demo.log" 2>/dev/null && [ "$ended" = "-2" ]; then
    ok "Ctrl-C: a command that dies of SIGINT makes the wrapper die of it too"
else
    fail "Ctrl-C: a command that dies of SIGINT makes the wrapper die of it too" \
        "ended $ended, file: $(cat "$logs/latest/demo.log" 2>&1)"
fi

cat >"$work/graceful.sh" <<'CMD'
trap 'echo stopped cleanly; exit 0' INT
echo started
while :; do sleep 0.1; done
CMD
logs="$work/logs-graceful"
ended="$(interrupt "$logs" "$work/graceful.sh")"
if grep -qx "stopped cleanly" "$logs/latest/demo.log" 2>/dev/null && [ "$ended" = "0" ]; then
    ok "Ctrl-C: a command that shuts down cleanly passes its 0 on"
else
    fail "Ctrl-C: a command that shuts down cleanly passes its 0 on" \
        "ended $ended, file: $(cat "$logs/latest/demo.log" 2>&1)"
fi

if [ "$failures" -gt 0 ]; then
    echo "$failures dev-log test(s) failed"
    exit 1
fi
echo "all dev-log tests pass"
