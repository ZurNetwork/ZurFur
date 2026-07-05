#!/usr/bin/env bash
#
# pds-provision.sh — create (or, if it already exists, sign into) the fixture
# test account on the local dev PDS (ZMVP-102), and print the resulting
# session as JSON on stdout.
#
# Idempotent by design: safe to run again after a `just pds-reset`/`db-reset`
# wipe (fresh account), or against an account that's already there (falls
# back to signing in) — that's what makes it usable both interactively and
# from the smoke test (scripts/pds-smoke-test.sh).
#
# Reads, ZURFUR_ convention (env wins, see .env.example):
#   ZURFUR_PDS_ENDPOINT       - the PDS's base URL. THE config key ZMVP-103's
#                               test-support crate and ZMVP-105 share.
#   ZURFUR_PDS_TEST_HANDLE    - fixture account handle (e.g. alice.test)
#   ZURFUR_PDS_TEST_EMAIL     - fixture account email
#   ZURFUR_PDS_TEST_PASSWORD  - fixture account password
#
# Exit 0 with the session JSON on stdout on success; exit 1 with the server's
# error body on stderr otherwise.
set -euo pipefail

endpoint="${ZURFUR_PDS_ENDPOINT:?ZURFUR_PDS_ENDPOINT is not set (see .env.example)}"
handle="${ZURFUR_PDS_TEST_HANDLE:?ZURFUR_PDS_TEST_HANDLE is not set}"
email="${ZURFUR_PDS_TEST_EMAIL:?ZURFUR_PDS_TEST_EMAIL is not set}"
password="${ZURFUR_PDS_TEST_PASSWORD:?ZURFUR_PDS_TEST_PASSWORD is not set}"

# A trailing slash on the endpoint would double up in the URLs below.
endpoint="${endpoint%/}"

# $1 = xrpc method, $2 = JSON body. Sets globals BODY / STATUS. Note this
# must NOT be invoked as the read side of a pipe (`foo | http_call ...`) —
# the assignments would be lost to a subshell.
http_call() {
    local response
    response="$(curl -sS -m 15 -w '\n%{http_code}' -X POST "$endpoint/xrpc/$1" \
        -H 'Content-Type: application/json' \
        -d "$2")"
    STATUS="${response##*$'\n'}"
    BODY="${response%$'\n'*}"
}

create_body=$(printf '{"handle":"%s","email":"%s","password":"%s"}' "$handle" "$email" "$password")
http_call com.atproto.server.createAccount "$create_body"

if [ "$STATUS" = "200" ]; then
    echo "$BODY"
    exit 0
fi

# Most likely cause of a non-200 here: the fixture account already exists
# from a prior provision (no wipe in between) — fall back to signing in so
# this stays idempotent either way.
session_body=$(printf '{"identifier":"%s","password":"%s"}' "$handle" "$password")
http_call com.atproto.server.createSession "$session_body"

if [ "$STATUS" = "200" ]; then
    echo "$BODY"
    exit 0
fi

echo "pds-provision: failed to create or sign into '$handle' on $endpoint" >&2
echo "  createAccount/createSession last response (HTTP $STATUS):" >&2
echo "  $BODY" >&2
exit 1
