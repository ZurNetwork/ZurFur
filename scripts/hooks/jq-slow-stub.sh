#!/bin/sh
# Test double: behaves exactly like the real `jq` for every call EXCEPT the
# final "-n --rawfile" call that builds additionalContext, where it sleeps
# well past any reasonable test timeout. Used by design-refs.test.sh to
# freeze the hook right after it has written its body temp file, so a
# SIGTERM sent at that moment can be asserted to still clean the file up.
case "$*" in
    *--rawfile*) sleep 30 ;;
esac
exec jq "$@"
