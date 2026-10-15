# log-files.sh: sourced by dev-log.sh, compose-logs.sh and logs.sh (bash).
# The dev logs live under one root, logs/ at the checkout's root unless
# ZURFUR_LOG_DIR says otherwise: logs/<dir>/<UTC stamp>.log, the newest 10 kept
# per directory, and logs/latest/<name>.log linking to each name's newest file.

# The marker file at a logs/ root: logs-clean removes only a root that has it.
LOG_ROOT_MARKER=".zurfur-logs"

# The logs/ root.
log_root() {
    local checkout
    checkout="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
    printf '%s\n' "${ZURFUR_LOG_DIR:-$checkout/logs}"
}

# log_prepare_root <root>: creates <root> and its latest/, and marks <root> as a
# logs/ root.
log_prepare_root() {
    mkdir -p "$1/latest" && : >>"$1/$LOG_ROOT_MARKER"
}

# log_name_ok <name>: whether <name> is a plain log name (no slash, no leading dot).
log_name_ok() {
    case "$1" in
        "" | *[!A-Za-z0-9._-]* | .*) return 1 ;;
    esac
    return 0
}

# log_claim <directory>: creates a fresh, empty log file there and prints its
# basename: the stamp, or the stamp with _2, _3… when a run in the same second
# took it. noclobber makes each claim atomic.
log_claim() {
    local directory="$1" stamp file suffix=1
    stamp="$(date -u +%Y%m%dT%H%M%SZ)"
    file="$stamp.log"
    while ! (set -C && : >"$directory/$file") 2>/dev/null; do
        suffix=$((suffix + 1))
        file="${stamp}_$suffix.log"
        if [ "$suffix" -gt 1000 ]; then
            echo "log-files: can't create a log file in $directory" >&2
            return 1
        fi
    done
    printf '%s\n' "$file"
}

# log_link <root> <name> <target relative to latest/>: points latest/<name>.log
# at <target>, swapped in whole so a reader never finds it missing. -T replaces
# the link itself, even one pointing at a directory.
log_link() {
    local root="$1" name="$2" target="$3"
    ln -sfn "$target" "$root/latest/.$name.log.$$" &&
        mv -fT "$root/latest/.$name.log.$$" "$root/latest/$name.log"
}

# log_retain <directory>: deletes all but the newest 10 log files there (stamps
# sort in time order).
log_retain() {
    local directory="$1" old
    find "$directory" -maxdepth 1 -type f -name '*.log' -exec basename {} \; |
        LC_ALL=C sort | head -n -10 | while read -r old; do
        rm -f "$directory/$old"
    done
}
