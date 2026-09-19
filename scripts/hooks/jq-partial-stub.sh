#!/bin/sh
# Test double: behaves exactly like the real `jq` for every call EXCEPT the
# final "-n --rawfile" call, where it writes a truncated fragment of a JSON
# document to stdout and then exits 1 — simulating a jq that dies mid-write.
# Used to prove the hook never lets that partial fragment reach real stdout.
case "$*" in
    *--rawfile*)
        printf '{"hookSpecificOutput": {"hookEve'
        exit 1
        ;;
esac
exec jq "$@"
