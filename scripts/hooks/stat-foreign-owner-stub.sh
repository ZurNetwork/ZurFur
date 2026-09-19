#!/bin/sh
# Test double for `stat`: always reports a uid that is NOT the current
# user's (real uid + 12345, so it's wrong regardless of who runs this),
# for pins.test.sh's "local pins file owned by someone else" case —
# independent of the symlink check, which takes a different code path.
case "${1:-}" in
    -c | -f) echo "$(($(id -u) + 12345))" ;;
    *) exit 1 ;;
esac
