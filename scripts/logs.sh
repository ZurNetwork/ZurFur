#!/usr/bin/env bash
# logs.sh tail [name] | clean: the dev logs' reader and sweeper.
#
# tail: `tail -F` every logs/latest/*.log, or only <name>'s; an unknown name
# lists the names there are and exits 1.
# clean: stops the compose followers, then removes logs/ and says what went.
set -uo pipefail
# shellcheck source=log-files.sh
. "$(dirname "${BASH_SOURCE[0]}")/log-files.sh"

root="$(log_root)"

# The names under latest/, one per line, sorted.
known_names() {
    local link
    for link in "$root"/latest/*.log; do
        [ -e "$link" ] || [ -L "$link" ] || continue
        basename "$link" .log
    done | LC_ALL=C sort
}

case "${1:-}" in
    tail)
        name="${2:-}"
        if [ -z "$name" ]; then
            mapfile -t names < <(known_names)
            if [ "${#names[@]}" -eq 0 ]; then
                echo "logs-tail: no logs under $root yet" >&2
                exit 1
            fi
            files=()
            for known in "${names[@]}"; do
                files+=("$root/latest/$known.log")
            done
            exec tail -F "${files[@]}"
        fi
        if ! log_name_ok "$name" || ! known_names | grep -qxF -- "$name"; then
            echo "logs-tail: no log named '$name'; the names are:" >&2
            known_names | sed 's/^/  /' >&2
            exit 1
        fi
        exec tail -F "$root/latest/$name.log"
        ;;
    clean)
        bash "$(dirname "${BASH_SOURCE[0]}")/compose-logs.sh" stop
        if [ ! -e "$root" ]; then
            echo "logs-clean: nothing to remove at $root"
            exit 0
        fi
        count="$(find "$root" -type f -name '*.log' | wc -l | tr -d ' ')"
        rm -rf "$root"
        echo "logs-clean: removed $root ($count log files)"
        ;;
    *)
        echo "usage: logs.sh tail [name] | clean" >&2
        exit 64
        ;;
esac
