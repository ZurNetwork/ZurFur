#!/bin/sh
# Test double for `stat`: always fails, as if stat itself were
# unavailable, permission-denied, or errored for any other reason.
# pins.sh must treat this as "ownership not proven" and skip the local
# pins file entirely — never fall through to "assume it's fine".
exit 1
