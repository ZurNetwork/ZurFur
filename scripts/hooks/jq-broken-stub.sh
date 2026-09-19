#!/bin/sh
# Test double for a catastrophically broken `jq`: exits 2 (jq's own usual
# error code) on every invocation, regardless of arguments, producing no
# stdout. Used by design-refs.test.sh via ZURFUR_JQ_BIN to prove the hook
# still exits 0 with empty stdout when jq itself is broken at runtime (as
# opposed to simply missing, which is covered separately).
exit 2
