#!/usr/bin/env bash
# dev-log.sh <name> <command…>: runs <command> with its output on the terminal
# as today and in logs/<name>/<UTC stamp>.log at the checkout's root, as plain
# text (no ANSI escapes, no carriage returns). logs/latest/<name>.log links to
# the newest file, each directory keeps its newest 10, and files are 0600.
# ZURFUR_LOG_DIR moves the logs/ root. The exit status is the command's; a
# command that dies of a Ctrl-C makes the wrapper die of the same SIGINT.
#
# A pipe, not script(1): a SIGINT to script(1) kills its child outright. Here
# tee and the stripper ignore SIGINT, so a Ctrl-C reaches only the command and
# the file still gets its last lines. Never wrap a command that prints a secret
# (pds-provision prints PDS session tokens).
set -uo pipefail
shopt -s lastpipe
umask 077
# shellcheck source=log-files.sh
. "$(dirname "${BASH_SOURCE[0]}")/log-files.sh"

if [ "$#" -lt 2 ]; then
    echo "usage: dev-log.sh <name> <command…>" >&2
    exit 64
fi
name="$1"
shift
if ! log_name_ok "$name"; then
    echo "dev-log.sh: '$name' is not a log name (letters, digits, '.', '_', '-')" >&2
    exit 64
fi

root="$(log_root)"
dir="$root/$name"
log_prepare_root "$root" || exit 1
mkdir -p "$dir" || exit 1
file="$(log_claim "$dir")" || exit 1
log_link "$root" "$name" "../$name/$file"
log_retain "$dir"

# Colors survive the pipe when a person is watching: cargo's, node's and an
# inner just's.
if [ -t 1 ]; then
    export CARGO_TERM_COLOR="${CARGO_TERM_COLOR:-always}" FORCE_COLOR="${FORCE_COLOR:-1}"
    export JUST_COLOR="${JUST_COLOR:-always}"
fi

# The file side: OSC and CSI sequences, other escapes and carriage returns go;
# lines are flushed as they come, for `tail -F`. Under the INT trap below, bash
# 5.3 already starts it with SIGINT ignored (measured: SigIgn 0x86); perl's own
# IGNORE keeps it deaf to a Ctrl-C whatever the shell does.
strip() {
    exec perl -pe 'BEGIN { $| = 1; $SIG{INT} = "IGNORE" }
        s/\e\][^\a\e]*(?:\a|\e\\)//g;
        s/\e\[[0-?]*[ -\/]*[@-~]//g;
        s/\e[()][0-9A-Za-z]//g;
        s/\e[@-Z\\-_=>78c]//g;
        s/\r//g;'
}

# A Ctrl-C reaches the command; the wrapper notes it and waits for the file side
# to finish. If the command died of it (status 128 or above), the wrapper dies of
# the same SIGINT so its caller sees how the run ended; a command that shut down
# cleanly on it passes its own status on.
interrupted=0
trap 'interrupted=1' INT
"$@" 2>&1 | tee -i >(strip >>"$dir/$file")
status=$?
wait "$!" 2>/dev/null
if [ "$interrupted" = 1 ] && [ "$status" -ge 128 ]; then
    trap - INT
    kill -INT "$$"
fi
exit "$status"
