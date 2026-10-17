#!/usr/bin/env bash
# Fills your Den on the dev stack with two commissions, as you: a live
# "Sketch request" and an archived "Old badge", each with two small PNG files
# and its slots. Every write goes through the API's commission routes with
# your session; the one other read is the commission's skeleton tab id, which
# no route hands out, read from the database with one read-only SELECT.
#
# Usage: just den-fill (or bash scripts/den-fill.sh), then paste the value of
# your zurfur.sid cookie at the hidden prompt. It reads the value from stdin
# when stdin isn't a terminal. Run it once: each run adds two more commissions.
#
# Environment:
#   ZURFUR_API_UPSTREAM  the API's internal origin (default http://127.0.0.1:8081);
#                        plain http only on loopback, otherwise https
#   ZURFUR_PUBLIC_URL    the browser-visible origin (default http://127.0.0.1:8080),
#                        sent as every call's Origin header; a bare origin only:
#                        https, or http on 127.0.0.1 or [::1], with an optional port
#   ZURFUR_FILL_PSQL     the psql command the tab read is piped into (default
#                        `docker compose exec -T db psql -U admin -d zurfur -tA`)
#
# The cookie stays in an unexported shell variable. It reaches curl only on
# stdin, as a `--config -` line the printf builtin writes into the pipe: never
# in an argument, the environment, a file or a message. curl runs with
# --disable (no ~/.curlrc), never verbose, and with the public origin as its
# Origin header, an argument checked to be a bare origin before the cookie is
# read. xtrace and allexport are turned off first, whatever the caller's
# SHELLOPTS turned on.
set +x +a
set -euo pipefail

readonly default_api="http://127.0.0.1:8081"
readonly default_origin="http://127.0.0.1:8080"
readonly default_psql="docker compose exec -T db psql -U admin -d zurfur -tA"
readonly uuid_pattern='^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
readonly loopback_http_pattern='^http://(127\.0\.0\.1|localhost|\[::1\])(:[0-9]+)?$'
readonly https_origin_pattern='^https://[A-Za-z0-9.-]+(:[0-9]{1,5})?$'
readonly loopback_origin_pattern='^http://(127\.0\.0\.1|\[::1\])(:[0-9]{1,5})?$'
readonly cookie_value_pattern='^[A-Za-z0-9._~+/=%-]{1,512}$'
readonly plain_path_pattern='^[A-Za-z0-9._/-]+$'

# Two 1x1 PNG images, a warm and a cool pixel.
readonly warm_png="iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR42mN4UBUPAAP3Abr1piyWAAAAAElFTkSuQmCC"
readonly cool_png="iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR42mOwjWoAAAHvARgAMwSbAAAAAElFTkSuQmCC"

say() { printf 'den-fill: %s\n' "$*" >&2; }
# Once the cookie is read, every stop reminds you to end that session.
cookie_read=""
die() {
    say "$*"
    if [[ -n $cookie_read ]]; then
        say "Sign out in the web app: it deletes this session."
    fi
    exit 1
}

for tool in curl jq base64 mktemp; do
    command -v "$tool" >/dev/null 2>&1 || die "needs $tool on PATH"
done

api="${ZURFUR_API_UPSTREAM:-$default_api}"
api="${api%/}"
if [[ $api != https://* && ! $api =~ $loopback_http_pattern ]]; then
    die "ZURFUR_API_UPSTREAM must be https, or http on loopback: $api"
fi
origin="${ZURFUR_PUBLIC_URL:-$default_origin}"
origin="${origin%/}"
if [[ ! $origin =~ $https_origin_pattern && ! $origin =~ $loopback_origin_pattern ]]; then
    die "ZURFUR_PUBLIC_URL must be a bare origin, https or http on 127.0.0.1 or [::1]: $origin"
fi
read -r -a psql_command <<<"${ZURFUR_FILL_PSQL:-$default_psql}"

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
[[ $work =~ $plain_path_pattern ]] || die "the temporary folder's path has characters curl's form syntax can't carry: $work"
printf '%s' "$warm_png" | base64 -d >"$work/warm.png"
printf '%s' "$cool_png" | base64 -d >"$work/cool.png"

# A same-named variable exported by the caller must not carry the cookie to a child.
unset -v session_cookie
session_cookie=""
read -r -s -p "Paste the value of your zurfur.sid cookie (hidden): " session_cookie || true
cookie_read=yes
[[ -t 0 ]] && printf '\n' >&2
session_cookie="${session_cookie%$'\r'}"
session_cookie="${session_cookie#zurfur.sid=}"
if [[ ! $session_cookie =~ $cookie_value_pattern ]]; then
    die "that isn't a zurfur.sid value: copy the cookie's value from devtools and run again"
fi

# The curl config line carrying the cookie, written by the printf builtin.
cookie_config() {
    printf 'cookie = "zurfur.sid=%s"\n' "$session_cookie"
}

# request <config writer> <method> <path> [curl args...]: sets status and body.
request() {
    local writer="$1" method="$2" path="$3"
    shift 3
    local reply
    if ! reply="$("$writer" | curl --disable --silent --show-error --noproxy '*' \
        --connect-timeout 5 --max-time 60 --config - --header "Origin: $origin" \
        --request "$method" --write-out '\n%{http_code}' "$@" --url "$api$path")"; then
        die "$method $path: the API at $api didn't answer"
    fi
    status="${reply##*$'\n'}"
    body="${reply%$'\n'*}"
}

# The problem's code from the last reply, or "no problem code".
problem_code() {
    local code
    code="$(printf '%s' "$body" | jq -r '.code? // empty' 2>/dev/null || true)"
    printf '%s' "${code:-no problem code}"
}

# expect <status> <method> <path>: stops unless the last reply had <status>.
expect() {
    [[ $status == "$1" ]] || die "$2 $3 answered $status ($(problem_code))"
}

# create_commission <title>: prints the new commission's id.
create_commission() {
    local title="$1" payload id
    payload="$(jq -cn --arg title "$title" '{title: $title}')"
    request cookie_config POST /commissions \
        --header 'Content-Type: application/json' --data-binary "$payload"
    expect 201 POST /commissions
    id="$(printf '%s' "$body" | jq -r '.id // empty')"
    [[ $id =~ $uuid_pattern ]] || die "POST /commissions answered no commission id"
    printf '%s' "$id"
}

# upload <commission id> <png file> <file name>
upload() {
    local commission="$1"
    upload_form="file=@$2;type=image/png;filename=$3"
    request upload_config POST "/commissions/$commission/files"
    expect 201 POST "/commissions/$commission/files"
}

# The upload's config: the cookie line and the multipart part.
upload_config() {
    cookie_config
    printf 'form = "%s"\n' "$upload_form"
}

# skeleton_tab <commission id>: prints the id of the commission's skeleton tab, read
# with one read-only SELECT, since no route hands it out.
skeleton_tab() {
    local commission="$1" rows tab
    [[ $commission =~ $uuid_pattern ]] || die "not a commission id: $commission"
    if ! rows="$(printf "BEGIN READ ONLY;\nSELECT id FROM commission_tab WHERE commission_id = '%s' AND tab = 'main';\nCOMMIT;\n" \
        "$commission" | "${psql_command[@]}")"; then
        die "the tab read failed: is the dev stack's database up (just up)?"
    fi
    tab="$(printf '%s\n' "$rows" | grep -E "$uuid_pattern" || true)"
    [[ $tab =~ $uuid_pattern ]] || die "commission $commission has no skeleton tab"
    printf '%s' "$tab"
}

# declare_slots <commission id> <slot title>...
declare_slots() {
    local commission="$1" tab payload
    shift
    tab="$(skeleton_tab "$commission")"
    payload="$(jq -cn --arg tab "$tab" '$ARGS.positional | map({tab: $tab, surface: "content", title: .})' --args "$@")"
    request cookie_config POST "/commissions/$commission/slots" \
        --header 'Content-Type: application/json' --data-binary "$payload"
    expect 201 POST "/commissions/$commission/slots"
}

# archive <commission id>
archive() {
    request cookie_config POST "/commissions/$1/archive"
    expect 204 POST "/commissions/$1/archive"
}

request cookie_config GET /me
if [[ $status == 401 ]]; then
    die "GET /me answered $status ($(problem_code)): sign in first, then copy zurfur.sid again"
fi
expect 200 GET /me

live="$(create_commission "Sketch request")"
upload "$live" "$work/warm.png" "reference.png"
upload "$live" "$work/cool.png" "pose.png"
declare_slots "$live" "Main character" "Second character"

old="$(create_commission "Old badge")"
upload "$old" "$work/cool.png" "badge-draft.png"
upload "$old" "$work/warm.png" "badge-final.png"
declare_slots "$old" "Badge wearer"
archive "$old"

printf 'Created in your Den:\n'
printf '  Sketch request  %s  live: reference.png, pose.png; slots Main character, Second character\n' "$live"
printf '  Old badge       %s  archived: badge-draft.png, badge-final.png; slot Badge wearer\n' "$old"
printf 'Open My Den > commissions (the archived one shows with deleted ones).\n'
printf 'When you are done: Sign out in the web app. It deletes this session.\n'
