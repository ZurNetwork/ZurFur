#!/usr/bin/env bash
# compose-logs.sh start <since> [service…] | stop [service…]
#
# start: for each compose service (default: db plc pds caddy), stops its old
# follower, then runs one detached `docker compose logs -f --no-color -t
# --since <since> <service>` under setsid into logs/compose/<service>/<stamp>.log
# (latest at logs/latest/<service>.log, the newest 10 kept), leaving a pidfile.
# A follower ends when its container stops.
# stop: ends the followers of the services named (default: every one with a
# pidfile). A pidfile's process is signalled only while it is still the one
# that was started: same pid, same start time.
set -uo pipefail
umask 077
# shellcheck source=log-files.sh
. "$(dirname "${BASH_SOURCE[0]}")/log-files.sh"

default_services=(db plc pds caddy)
root="$(log_root)"
compose="$root/compose"

usage() {
    echo "usage: compose-logs.sh start <since> [service…] | stop [service…]" >&2
    exit 64
}

# The start time ps reports for <pid>, or nothing when it's gone.
started_at() {
    ps -o lstart= -p "$1" 2>/dev/null | tr -s ' '
}

# stop_follower <service>: ends its follower's process group if the pidfile still
# names it, and removes the pidfile either way.
stop_follower() {
    local pidfile="$compose/$1/follower.pid" pid recorded
    [ -f "$pidfile" ] || return 0
    read -r pid recorded <"$pidfile" || true
    if [ -n "${pid:-}" ] && [ -n "$(started_at "$pid")" ] &&
        [ "$(started_at "$pid" | tr ' ' '_')" = "${recorded:-}" ]; then
        if [ "$(ps -o pgid= -p "$pid" | tr -d ' ')" = "$pid" ]; then
            kill -TERM -- "-$pid" 2>/dev/null || true
        else
            kill -TERM "$pid" 2>/dev/null || true
        fi
    fi
    rm -f "$pidfile"
}

# start_follower <since> <service>
start_follower() {
    local since="$1" service="$2" directory file pid
    directory="$compose/$service"
    log_prepare_root "$root" || return 1
    mkdir -p "$directory" || return 1
    stop_follower "$service"
    file="$(log_claim "$directory")" || return 1
    log_link "$root" "$service" "../compose/$service/$file"
    log_retain "$directory"
    # Never add --verbose: podman-compose then logs its `podman run -e …` lines,
    # which carry the PDS's secrets (its JWT secret, admin password, rotation key).
    setsid docker compose logs -f --no-color -t --since "$since" "$service" \
        >>"$directory/$file" 2>&1 </dev/null &
    pid=$!
    printf '%s %s\n' "$pid" "$(started_at "$pid" | tr ' ' '_')" >"$directory/follower.pid"
}

# Every service with a pidfile.
followed_services() {
    local pidfile
    for pidfile in "$compose"/*/follower.pid; do
        [ -f "$pidfile" ] && basename "$(dirname "$pidfile")"
    done
}

[ "$#" -ge 1 ] || usage
command="$1"
shift
case "$command" in
    start)
        [ "$#" -ge 1 ] || usage
        since="$1"
        shift
        services=("$@")
        [ "${#services[@]}" -gt 0 ] || services=("${default_services[@]}")
        for service in "${services[@]}"; do
            log_name_ok "$service" || usage
            start_follower "$since" "$service"
        done
        ;;
    stop)
        services=("$@")
        [ "${#services[@]}" -gt 0 ] || mapfile -t services < <(followed_services)
        for service in "${services[@]}"; do
            log_name_ok "$service" || usage
            stop_follower "$service"
        done
        ;;
    *) usage ;;
esac
