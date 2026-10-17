#!/bin/sh
# Tests for scripts/den-fill.sh against a recording `curl` shim and a fake
# psql. Run directly, or via `just scripts-test`. Needs bash and jq.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/den-fill.sh"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

cookie="TestSidValueOnStdin_42-xyz"

# The shim records each call into $SHIM_DIR/<n>.{argv,env,stdin,tmpfiles} (the
# last: every file under $TMPDIR, its name and contents) and answers by path.
# me.status, if present, is GET /me's status (default 200); refuse names the one
# request answered 422 unknown_surface; unreachable names the one where curl
# exits 7, as when nothing listens.
mkdir -p "$work/bin"
cat >"$work/bin/curl" <<'SHIM'
#!/bin/sh
dir="$SHIM_DIR"
n=$(($(cat "$dir/calls" 2>/dev/null || echo 0) + 1))
echo "$n" >"$dir/calls"
for arg in "$@"; do printf '%s\n' "$arg"; done >"$dir/$n.argv"
env >"$dir/$n.env"
cat >"$dir/$n.stdin"
find "$TMPDIR" -type f -print -exec cat {} + >"$dir/$n.tmpfiles" 2>/dev/null
method=GET
url=""
prev=""
for arg in "$@"; do
    case "$prev" in
        --request) method="$arg" ;;
        --url) url="$arg" ;;
    esac
    prev="$arg"
done
path="${url#http://127.0.0.1:8081}"
echo "$method $path" >>"$dir/requests"
if [ -f "$dir/unreachable" ] && [ "$method $path" = "$(cat "$dir/unreachable")" ]; then
    echo "curl: (7) Failed to connect to 127.0.0.1 port 8081" >&2
    exit 7
fi
if [ -f "$dir/refuse" ] && [ "$method $path" = "$(cat "$dir/refuse")" ]; then
    printf '{"type":"urn:zurfur:error:unknown-surface","code":"unknown_surface","title":"t","detail":"d","status":422}\n422'
    exit 0
fi
case "$method $path" in
    "GET /me")
        status=$(cat "$dir/me.status" 2>/dev/null || echo 200)
        if [ "$status" = 200 ]; then
            printf '{"did":"did:plc:fillerfixtureaaaaaaaaaaa"}\n%s' "$status"
        else
            printf '{"type":"urn:zurfur:error:not-authenticated","code":"not_authenticated","title":"t","detail":"d","status":401}\n%s' "$status"
        fi
        ;;
    "POST /commissions")
        printf '{"id":"0190f000-0000-7000-8000-00000000000%s"}\n201' "$n"
        ;;
    "POST /commissions/"*/files)
        printf '{"id":"0190f000-0000-7000-8000-0000000001%02d"}\n201' "$n"
        ;;
    "POST /commissions/"*/slots)
        printf '{"ids":["0190f000-0000-7000-8000-000000000200"]}\n201'
        ;;
    "POST /commissions/"*/archive)
        printf '\n204'
        ;;
    *)
        printf '{"code":"not_found"}\n404'
        ;;
esac
SHIM
chmod +x "$work/bin/curl"

# The fake psql records its stdin and environment and answers one tab id.
cat >"$work/bin/fake-psql" <<'SHIM'
#!/bin/sh
n=$(($(cat "$SHIM_DIR/psql-calls" 2>/dev/null || echo 0) + 1))
echo "$n" >"$SHIM_DIR/psql-calls"
cat >"$SHIM_DIR/psql-$n.stdin"
env >"$SHIM_DIR/psql-$n.env"
find "$TMPDIR" -type f -print -exec cat {} + >"$SHIM_DIR/psql-$n.tmpfiles" 2>/dev/null
printf 'BEGIN\n0190f000-0000-7000-8000-0000000000aa\nCOMMIT\n'
SHIM
chmod +x "$work/bin/fake-psql"

failures=0
fail() {
    echo "FAIL: $*"
    failures=$((failures + 1))
}
pass() { echo "ok: $*"; }
# section: remembers the failure count, so passed_section passes only a clean section.
section() { section_failures=$failures; }
passed_section() {
    if [ "$failures" = "$section_failures" ]; then pass "$*"; fi
}

# fill <case dir> <stdin text> [env assignments...]: runs the script with the
# shims on PATH, the cookie on stdin, its own TMPDIR and a caller-exported
# `session_cookie` that must not carry the real one; records the exit status in
# <case>/exit. ZURFUR_PUBLIC_URL is unset unless a case sets it, so the one an
# outer `just` loads from .env never leaks in.
fill() {
    case_dir="$1"
    input="$2"
    shift 2
    mkdir -p "$case_dir/tmp"
    status=0
    printf '%s\n' "$input" | env -u ZURFUR_PUBLIC_URL PATH="$work/bin:$PATH" SHIM_DIR="$case_dir" TMPDIR="$case_dir/tmp" \
        ZURFUR_API_UPSTREAM=http://127.0.0.1:8081 ZURFUR_FILL_PSQL="$work/bin/fake-psql" \
        session_cookie=caller-exported "$@" \
        bash "$under_test" >"$case_dir/out" 2>"$case_dir/err" || status=$?
    echo "$status" >"$case_dir/exit"
}

# holds <file...>: whether any of the files contains the cookie.
holds() { grep -qF -- "$cookie" "$@" 2>/dev/null; }

# origin_once <argv file> <origin>: whether the argv names an Origin exactly once,
# as the one argument `Origin: <origin>` right after a --header.
origin_once() {
    awk -v want="Origin: $2" '
        tolower($0) ~ /^[[:space:]]*origin[[:space:]]*:/ {
            n++
            if ($0 != want || prev != "--header") bad = 1
        }
        { prev = $0 }
        END { exit (n == 1 && !bad) ? 0 : 1 }' "$1"
}

# no_stdin_origin <stdin file>: whether the curl config carries no Origin.
no_stdin_origin() { ! grep -qi 'origin' "$1"; }

# --- A full fill: ten calls, the cookie only ever on curl's stdin.
run="$work/full"
fill "$run" "$cookie"
if [ "$(cat "$run/exit")" = 0 ]; then pass "a full fill exits 0"; else fail "a full fill exited $(cat "$run/exit"): $(cat "$run/err")"; fi
calls=$(cat "$run/calls" 2>/dev/null || echo 0)
if [ "$calls" = 10 ]; then pass "ten calls"; else fail "expected 10 calls, got $calls"; fi
expected_requests="GET /me
POST /commissions
POST /commissions/0190f000-0000-7000-8000-000000000002/files
POST /commissions/0190f000-0000-7000-8000-000000000002/files
POST /commissions/0190f000-0000-7000-8000-000000000002/slots
POST /commissions
POST /commissions/0190f000-0000-7000-8000-000000000006/files
POST /commissions/0190f000-0000-7000-8000-000000000006/files
POST /commissions/0190f000-0000-7000-8000-000000000006/slots
POST /commissions/0190f000-0000-7000-8000-000000000006/archive"
if [ "$(cat "$run/requests")" = "$expected_requests" ]; then pass "the routes, in order, the second commission archived"; else fail "requests were: $(cat "$run/requests")"; fi
section
i=1
while [ "$i" -le "$calls" ]; do
    holds "$run/$i.stdin" || fail "call $i: the cookie is not on stdin"
    holds "$run/$i.argv" && fail "call $i: the cookie is in argv"
    holds "$run/$i.env" && fail "call $i: the cookie is in the environment"
    holds "$run/$i.tmpfiles" && fail "call $i: a temporary file holds the cookie"
    grep -qvE '^(cookie|form) = "' "$run/$i.stdin" && fail "call $i: the config carries more than the cookie and the form"
    grep -qxE -- '-v|--verbose|--trace|--trace-ascii|--trace-time|--trace-config|-#|-D|--dump-header|-i|--include|-o|--output' "$run/$i.argv" &&
        fail "call $i: argv runs verbose, traces or writes the exchange out"
    [ "$(head -n1 "$run/$i.argv")" = "--disable" ] || fail "call $i: --disable is not the first argument"
    grep -qx -- '--config' "$run/$i.argv" || fail "call $i: no --config"
    awk 'prev == "--config" && $0 != "-" { bad = 1 } { prev = $0 } END { exit bad ? 0 : 1 }' "$run/$i.argv" &&
        fail "call $i: --config reads something other than stdin"
    i=$((i + 1))
done
passed_section "the cookie is on every call's stdin and in no argv or environment"
section
i=1
while [ "$i" -le "$calls" ]; do
    origin_once "$run/$i.argv" "http://127.0.0.1:8080" ||
        fail "call $i: argv doesn't send Origin: http://127.0.0.1:8080 once, as a --header argument: $(grep -i origin "$run/$i.argv")"
    no_stdin_origin "$run/$i.stdin" || fail "call $i: the config on stdin carries an Origin"
    i=$((i + 1))
done
passed_section "with ZURFUR_PUBLIC_URL unset, every call sends Origin: http://127.0.0.1:8080 as an argument, never on stdin"
section
grep -qF 'cookie = "zurfur.sid='"$cookie"'"' "$run/1.stdin" || fail "the config line is not cookie = \"zurfur.sid=...\""
grep -qF 'form = "file=@' "$run/3.stdin" || fail "the upload's form is not in its config"
for p in "$run"/psql-*.stdin; do
    holds "$p" "${p%.stdin}.env" "${p%.stdin}.tmpfiles" && fail "the psql read sees the cookie"
    grep -q '^BEGIN READ ONLY;$' "$p" || fail "the psql read is not a read-only transaction"
    [ "$(grep -c '^SELECT' "$p")" = 1 ] || fail "the psql read is not one SELECT"
    grep -qiE 'insert|update|delete|drop|alter' "$p" && fail "the psql read writes"
done
[ "$(cat "$run/psql-calls")" = 2 ] || fail "expected one tab read per commission"
holds "$run/out" "$run/err" && fail "the script's own output holds the cookie"
grep -q 'Sketch request' "$run/out" || fail "the summary lacks the live title"
grep -q 'Old badge' "$run/out" || fail "the summary lacks the archived title"
tail -n1 "$run/out" | grep -q 'Sign out' || fail "the summary doesn't end with the sign-out reminder"
passed_section "psql never sees the cookie; the output never holds it"

# --- Under bash -x the cookie still never reaches stderr.
traced="$work/traced"
mkdir -p "$traced/tmp"
status=0
printf '%s\n' "$cookie" | env -u ZURFUR_PUBLIC_URL PATH="$work/bin:$PATH" SHIM_DIR="$traced" TMPDIR="$traced/tmp" \
    ZURFUR_API_UPSTREAM=http://127.0.0.1:8081 ZURFUR_FILL_PSQL="$work/bin/fake-psql" \
    bash -x "$under_test" >"$traced/out" 2>"$traced/err" || status=$?
[ "$status" = 0 ] || fail "the traced fill exited $status"
if holds "$traced/out" "$traced/err"; then fail "bash -x prints the cookie"; else pass "bash -x never prints the cookie"; fi

# --- A refused /me stops before any write and says to sign in.
refused="$work/refused"
mkdir -p "$refused"
echo 401 >"$refused/me.status"
section
fill "$refused" "$cookie"
[ "$(cat "$refused/exit")" != 0 ] || fail "a refused /me exits 0"
[ "$(cat "$refused/requests")" = "GET /me" ] || fail "a refused /me went on to: $(cat "$refused/requests")"
[ -f "$refused/psql-calls" ] && fail "a refused /me still read the tab"
grep -q 'sign in first' "$refused/err" || fail "a refused /me doesn't say sign in first: $(cat "$refused/err")"
grep -q '401 (not_authenticated)' "$refused/err" || fail "a refused /me doesn't print its status and code"
grep -q 'Sign out' "$refused/err" || fail "a refused /me doesn't remind to sign out"
holds "$refused/out" "$refused/err" && fail "a refused /me prints the cookie"
passed_section "a refused /me stops before any write"

# --- A value that could inject a config line is refused before any call.
section
injected="$work/injected"
fill "$injected" "$cookie\"
output = \"/tmp/stolen"
[ "$(cat "$injected/exit")" != 0 ] || fail "a quoted value exits 0"
[ -f "$injected/calls" ] && fail "a quoted value still called curl"
holds "$injected/out" "$injected/err" && fail "a refused value is printed back"
quoted="$work/quoted"
fill "$quoted" "$cookie\";url=\"http://elsewhere"
[ -f "$quoted/calls" ] && fail "a value with a quote still called curl"
passed_section "a value that isn't a cookie is refused before any call"

# --- An empty value is refused before any call.
section
empty="$work/empty"
fill "$empty" ""
[ "$(cat "$empty/exit")" != 0 ] || fail "an empty value exits 0"
[ -f "$empty/calls" ] && fail "an empty value still called curl"
passed_section "an empty value is refused"

# --- Plain http off loopback is refused before the cookie is read.
section
remote="$work/remote"
fill "$remote" "$cookie" ZURFUR_API_UPSTREAM=http://example.com
[ "$(cat "$remote/exit")" != 0 ] || fail "plain http off loopback exits 0"
[ -f "$remote/calls" ] && fail "plain http off loopback still called curl"
passed_section "plain http off loopback is refused"

# --- A set ZURFUR_PUBLIC_URL is every call's Origin, one trailing / stripped.
for case in "set-loopback http://127.0.0.1:8180/ http://127.0.0.1:8180" \
    "set-https https://zurfur.example:8443 https://zurfur.example:8443"; do
    set -- $case
    section
    fill "$work/$1" "$cookie" ZURFUR_PUBLIC_URL="$2"
    [ "$(cat "$work/$1/exit")" = 0 ] || fail "ZURFUR_PUBLIC_URL=$2: the fill exited $(cat "$work/$1/exit"): $(cat "$work/$1/err")"
    n=$(cat "$work/$1/calls" 2>/dev/null || echo 0)
    [ "$n" = 10 ] || fail "ZURFUR_PUBLIC_URL=$2: expected 10 calls, got $n"
    i=1
    while [ "$i" -le "$n" ]; do
        origin_once "$work/$1/$i.argv" "$3" ||
            fail "ZURFUR_PUBLIC_URL=$2: call $i doesn't send Origin: $3 once: $(grep -i origin "$work/$1/$i.argv")"
        no_stdin_origin "$work/$1/$i.stdin" || fail "ZURFUR_PUBLIC_URL=$2: call $i's config carries an Origin"
        i=$((i + 1))
    done
    passed_section "ZURFUR_PUBLIC_URL=$2 is sent as Origin: $3 on every call"
done

# --- A ZURFUR_PUBLIC_URL that isn't a bare origin is refused before the cookie is read.
newline_origin="http://127.0.0.1:8080
X-Injected: 1"
cr_origin="$(printf 'http://127.0.0.1:8080\r')"
for bad in "http://127.0.0.1:8080/api" "http://127.0.0.1:8080?next=1" "http://example.com" \
    "http://localhost:8080" "https://zurfur.example/path" "$newline_origin" "$cr_origin"; do
    section
    bad_case="$work/bad-origin"
    rm -rf "$bad_case"
    fill "$bad_case" "$cookie" ZURFUR_PUBLIC_URL="$bad"
    shown="$(printf '%s' "$bad" | tr '\r\n' '^~')"
    [ "$(cat "$bad_case/exit")" != 0 ] || fail "ZURFUR_PUBLIC_URL=$shown exits 0"
    [ -f "$bad_case/calls" ] && fail "ZURFUR_PUBLIC_URL=$shown still called curl"
    grep -q 'ZURFUR_PUBLIC_URL must be a bare origin' "$bad_case/err" || fail "ZURFUR_PUBLIC_URL=$shown isn't refused by name: $(cat "$bad_case/err")"
    grep -q 'Sign out' "$bad_case/err" && fail "ZURFUR_PUBLIC_URL=$shown was refused after the cookie was read"
    passed_section "ZURFUR_PUBLIC_URL=$shown is refused before the cookie is read"
done

# --- An inherited allexport never exports the cookie.
section
exported="$work/allexport"
fill "$exported" "$cookie" SHELLOPTS=allexport
[ "$(cat "$exported/exit")" = 0 ] || fail "the allexport fill exited $(cat "$exported/exit"): $(cat "$exported/err")"
for e in "$exported"/*.env; do
    holds "$e" && fail "under allexport, $(basename "$e") holds the cookie"
done
passed_section "an inherited allexport never exports the cookie"

# --- A refused write stops, prints its status and code, and reminds to sign out.
section
refused_write="$work/refused-write"
mkdir -p "$refused_write"
echo "POST /commissions/0190f000-0000-7000-8000-000000000002/slots" >"$refused_write/refuse"
fill "$refused_write" "$cookie"
[ "$(cat "$refused_write/exit")" != 0 ] || fail "a refused write exits 0"
grep -q '422 (unknown_surface)' "$refused_write/err" || fail "a refused write doesn't print its status and code: $(cat "$refused_write/err")"
grep -q 'Sign out' "$refused_write/err" || fail "a refused write doesn't remind to sign out"
grep -q 'POST /commissions$' "$refused_write/requests" || fail "a refused write never got going"
[ "$(grep -c 'archive' "$refused_write/requests")" = 0 ] || fail "a refused write went on to archive"
holds "$refused_write/out" "$refused_write/err" && fail "a refused write prints the cookie"
passed_section "a refused write stops and never prints the cookie"

# --- An API that doesn't answer stops and never prints the cookie.
section
unreachable="$work/unreachable"
mkdir -p "$unreachable"
echo "POST /commissions" >"$unreachable/unreachable"
fill "$unreachable" "$cookie"
[ "$(cat "$unreachable/exit")" != 0 ] || fail "an unreachable API exits 0"
grep -q "didn't answer" "$unreachable/err" || fail "an unreachable API isn't named: $(cat "$unreachable/err")"
grep -q 'Sign out' "$unreachable/err" || fail "an unreachable API doesn't remind to sign out"
holds "$unreachable/out" "$unreachable/err" && fail "an unreachable API prints the cookie"
passed_section "an unreachable API stops and never prints the cookie"

# --- A /me that is neither 200 nor 401 also stops before any write.
section
odd_me="$work/odd-me"
mkdir -p "$odd_me"
echo 500 >"$odd_me/me.status"
fill "$odd_me" "$cookie"
[ "$(cat "$odd_me/exit")" != 0 ] || fail "a 500 /me exits 0"
[ "$(cat "$odd_me/requests")" = "GET /me" ] || fail "a 500 /me went on to: $(cat "$odd_me/requests")"
holds "$odd_me/out" "$odd_me/err" && fail "a 500 /me prints the cookie"
passed_section "a /me that isn't 200 stops before any write"

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all den-fill tests passed"
