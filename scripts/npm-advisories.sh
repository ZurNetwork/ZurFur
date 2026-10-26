#!/bin/sh
# Scans both npm lockfiles for published advisories with a pinned osv-scanner:
# frontend/yarn.lock against osv-scanner.frontend.toml and
# lexicons/package-lock.json against osv-scanner.lexicons.toml, run from the
# repository root (its current directory).
#
#   sh scripts/npm-advisories.sh [--frontend-config <file>] [--lexicons-config <file>]
#
# Order: both ignore files pass the allow-list, both lockfiles exist, the
# binary is verified (downloaded on first use), the self-test proves the
# scanner finds and ignores an advisory, then each lockfile is scanned once
# with its own config and its package count checked.
#
# Exit codes: 0 clean; 1 an advisory no ignore file accepts; 2 an ignore file
# breaks the allow-list, or bad usage; 3 the scanner couldn't check (download
# failed, self-test failed, a count mismatch, no pin for this platform, a
# missing lockfile, osv-scanner exiting anything but 0 or 1); 4 a downloaded
# binary's hash didn't match. With both lockfiles scanned, the worst (highest)
# code wins. NPM_ADVISORIES_MODE=report turns 1 and 3 into a ::warning:: line
# and exit 0; 2 and 4 always block, and any other value blocks.
#
# What leaves the machine: the public dependencies' names and versions (to
# osv.dev), and the first run's download from GitHub releases. Needs curl,
# sha256sum and python3 >= 3.11 (tomllib).
set -u

# The scanner's pin, Linux x86_64 only. Its provenance is unchecked: the
# release has no GitHub attestation and slsa-verifier wasn't available. Trust
# rests on the release as published, cross-checked three ways when it was
# pinned (the release's SHA256SUMS, the downloaded bytes, GitHub's asset
# digest); this committed hash then blocks a later swap of the file.
OSV_SCANNER_VERSION=2.7.0
OSV_SCANNER_SHA256=160c6253e2922e9e0a6a9771c5c4c40a4b3c6090b34a3cdd5e80c96cc2e4218b
OSV_SCANNER_URL="https://github.com/google/osv-scanner/releases/download/v$OSV_SCANNER_VERSION/osv-scanner_linux_amd64"

# The advisory the self-test expects in cookie 0.6.0.
SELF_TEST_ID=GHSA-pxg6-pf52-xh8x

mode="${NPM_ADVISORIES_MODE:-}"
part=""
run=""

say() { printf 'npm-advisories: %s\n' "$*" >&2; }

cleanup() {
    [ -n "$part" ] && rm -f "$part"
    [ -n "$run" ] && rm -rf "$run"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM

# finish <code>: exits with <code>, or, in report mode, turns 1 and 3 into a warning.
finish() {
    if [ "$mode" = report ]; then
        case "$1" in
            1) kind="an advisory no ignore file accepts" ;;
            3) kind="the scanner couldn't check" ;;
            *) kind="" ;;
        esac
        if [ -n "$kind" ]; then
            printf '::warning::npm-advisories: %s (exit %s); a PR changing frontend/yarn.lock or lexicons/package-lock.json, and just gate, block on it\n' "$kind" "$1"
            exit 0
        fi
    fi
    exit "$1"
}

usage() {
    say "usage: npm-advisories.sh [--frontend-config <file>] [--lexicons-config <file>]"
    exit 2
}

frontend_config=osv-scanner.frontend.toml
lexicons_config=osv-scanner.lexicons.toml
while [ "$#" -gt 0 ]; do
    case "$1" in
        --frontend-config)
            [ "$#" -ge 2 ] || usage
            frontend_config="$2"
            shift 2
            ;;
        --lexicons-config)
            [ "$#" -ge 2 ] || usage
            lexicons_config="$2"
            shift 2
            ;;
        *) usage ;;
    esac
done

# absolute <path>: <path> made absolute against the current directory.
absolute() {
    case "$1" in
        /*) printf '%s\n' "$1" ;;
        *) printf '%s/%s\n' "$root" "$1" ;;
    esac
}

root="$(pwd -P)"
frontend_config="$(absolute "$frontend_config")"
lexicons_config="$(absolute "$lexicons_config")"
frontend_lock="$root/frontend/yarn.lock"
lexicons_lock="$root/lexicons/package-lock.json"

# --- the allow-list, before any download or scan ------------------------------

if ! python3 -I -c 'import sys, tomllib; sys.exit(sys.version_info < (3, 11))' 2>/dev/null; then
    say "the allow-list needs python3 >= 3.11 (tomllib)"
    finish 3
fi

# allow_list <file>: exits 0 when <file> holds only IgnoredVulns entries of
# exactly id, reason and ignoreUntil (a TOML date at most 90 days out), else 2.
allow_list() {
    python3 -I - "$1" <<'PY'
import datetime
import sys
import tomllib

path = sys.argv[1]


def refuse(message):
    print(f"npm-advisories: {path}: {message}", file=sys.stderr)
    sys.exit(2)


try:
    with open(path, "rb") as handle:
        config = tomllib.load(handle)
except OSError as error:
    refuse(f"can't be read ({error.strerror})")
except tomllib.TOMLDecodeError as error:
    refuse(f"isn't valid TOML ({error})")

others = sorted(set(config) - {"IgnoredVulns"})
if others:
    refuse("the only key allowed is IgnoredVulns; found " + ", ".join(others))

entries = config.get("IgnoredVulns", [])
if not isinstance(entries, list) or not all(isinstance(entry, dict) for entry in entries):
    refuse("IgnoredVulns must be an array of tables ([[IgnoredVulns]])")

latest = datetime.date.today() + datetime.timedelta(days=90)
wanted = {"id", "reason", "ignoreUntil"}
seen = set()
for number, entry in enumerate(entries, start=1):
    where = f"IgnoredVulns entry {number}"
    unknown = sorted(set(entry) - wanted)
    if unknown:
        refuse(f"{where} has a key that isn't allowed: " + ", ".join(unknown))
    missing = sorted(wanted - set(entry))
    if missing:
        refuse(f"{where} lacks " + ", ".join(missing))
    vuln_id = entry["id"]
    if not isinstance(vuln_id, str) or not vuln_id.strip():
        refuse(f"{where}: id must be a non-empty string")
    where = f"{where} ({vuln_id})"
    if vuln_id in seen:
        refuse(f"{where}: the id is listed twice")
    seen.add(vuln_id)
    reason = entry["reason"]
    if not isinstance(reason, str) or not reason.strip():
        refuse(f"{where}: reason must be a non-empty string")
    until = entry["ignoreUntil"]
    if type(until) is not datetime.date:
        refuse(f"{where}: ignoreUntil must be a bare TOML date (YYYY-MM-DD), not {type(until).__name__}")
    if until > latest:
        refuse(f"{where}: ignoreUntil {until} is more than 90 days out (latest {latest})")
PY
}

allow_list "$frontend_config" || finish 2
allow_list "$lexicons_config" || finish 2

for lock in "$frontend_lock" "$lexicons_lock"; do
    if [ ! -f "$lock" ]; then
        say "no lockfile at $lock (run from the repository root)"
        finish 3
    fi
done

# --- the binary: pinned, verified on every run --------------------------------

platform="$(uname -s)-$(uname -m)"
if [ "$platform" != Linux-x86_64 ]; then
    say "no osv-scanner $OSV_SCANNER_VERSION pin for $platform (only Linux-x86_64 is pinned)"
    finish 3
fi

cache="${XDG_CACHE_HOME:-$HOME/.cache}/zurfur-tools"
if ! mkdir -p "$cache" || ! cache="$(cd "$cache" && pwd -P)"; then
    say "can't create the cache folder $cache"
    finish 3
fi
binary="$cache/osv-scanner-$OSV_SCANNER_VERSION"

# hash_ok <file>: whether <file>'s SHA-256 is the pin.
hash_ok() {
    sum="$(sha256sum "$1" | awk '{ print $1 }')"
    [ "$sum" = "$OSV_SCANNER_SHA256" ]
}

if [ -e "$binary" ] && ! hash_ok "$binary"; then
    say "$binary doesn't match its pin; deleting it and downloading again"
    rm -f "$binary"
fi
if [ ! -e "$binary" ]; then
    say "downloading osv-scanner $OSV_SCANNER_VERSION from $OSV_SCANNER_URL"
    if ! part="$(mktemp "$cache/.osv-scanner-$OSV_SCANNER_VERSION.XXXXXX")"; then
        say "can't create a download file in $cache"
        finish 3
    fi
    if ! curl -fsSL --proto '=https' --proto-redir '=https' -o "$part" "$OSV_SCANNER_URL"; then
        say "the download failed"
        finish 3
    fi
    if ! hash_ok "$part"; then
        say "the downloaded file's SHA-256 is $sum, not the pinned $OSV_SCANNER_SHA256; deleted it, ran nothing"
        finish 4
    fi
    chmod 755 "$part"
    mv -f "$part" "$binary"
    part=""
fi
if ! hash_ok "$binary"; then
    say "$binary doesn't match its pin; ran nothing"
    finish 4
fi
if [ ! -x "$binary" ]; then
    say "$binary isn't executable; ran nothing"
    finish 3
fi

# --- scanning -------------------------------------------------------------------

if ! run="$(mktemp -d "$cache/run.XXXXXX")" || ! mkdir "$run/home" "$run/tmp" "$run/self-test"; then
    say "can't create a temp folder in $cache"
    finish 3
fi

# scan <config> <lockfile> <expected count> <output file>: runs the scanner
# under a clean environment and sets scan_status to its exit code and
# count_seen to 1 when it reported <expected count> packages for <lockfile>.
scan() {
    env -i PATH=/usr/bin:/bin HOME="$run/home" TMPDIR="$run/tmp" \
        "$binary" scan source --config "$1" --lockfile "$2" >"$4" 2>&1
    scan_status=$?
    if [ "$3" = 1 ]; then noun=package; else noun=packages; fi
    count_seen=0
    grep -Fxq -- "Scanned $2 file and found $3 $noun" "$4" && count_seen=1
}

# osv_kind <code>: what osv-scanner (and the shell) mean by an exit code.
osv_kind() {
    case "$1" in
        126) echo "osv-scanner: a result error; the shell: found but not executable" ;;
        127) echo "osv-scanner: a general error; the shell: command not found (network?)" ;;
        128) echo "osv-scanner: no packages found" ;;
        *)
            if [ "$1" -ge 129 ]; then
                echo "osv-scanner: another error, or a signal (network?)"
            else
                echo "osv-scanner: a result error"
            fi
            ;;
    esac
}

# The self-test: a one-package lockfile with a known advisory, scanned with no
# ignore, with the advisory ignored until tomorrow, and until yesterday.
self_lock="$run/self-test/yarn.lock"
cat >"$self_lock" <<'LOCK'
# THIS IS AN AUTOGENERATED FILE. DO NOT EDIT THIS FILE DIRECTLY.
# yarn lockfile v1


cookie@0.6.0:
  version "0.6.0"
  resolved "https://registry.yarnpkg.com/cookie/-/cookie-0.6.0.tgz#2798b04b071b0ecbff0dbb62a505a8efa4e19051"
  integrity sha512-U71cyTamuh1CRNCfpGY6to28lxvNwPG4Guz/EVjgf3Jmzv0vlDp1atT9eS5dDjMYHucpHbWns6Lwf3BKz6svdw==
LOCK
: >"$run/self-test-none.toml"
# ignored_until <date> <file>: an ignore file accepting the self-test's advisory until <date>.
ignored_until() {
    printf '[[IgnoredVulns]]\nid = "%s"\nignoreUntil = %s\nreason = "self-test"\n' "$SELF_TEST_ID" "$1" >"$2"
}
ignored_until "$(date -d '+1 day' +%F)" "$run/self-test-tomorrow.toml"
ignored_until "$(date -d '-1 day' +%F)" "$run/self-test-yesterday.toml"

# self_test <case> <expected exit> <whether the output names the advisory>
self_test() {
    out="$run/self-test-$1.out"
    scan "$run/self-test-$1.toml" "$self_lock" 1 "$out"
    named=0
    grep -Fq -- "$SELF_TEST_ID" "$out" && named=1
    if [ "$scan_status" != "$2" ] || [ "$count_seen" != 1 ] || { [ "$3" = 1 ] && [ "$named" != 1 ]; }; then
        say "self-test failed: cookie 0.6.0 with the ignore '$1' exited $scan_status (wanted $2), count line seen $count_seen, $SELF_TEST_ID named $named; its output:"
        cat "$out" >&2
        finish 3
    fi
}
self_test none 1 1
self_test tomorrow 0 0
self_test yesterday 1 1
say "self-test passed: osv-scanner $OSV_SCANNER_VERSION finds $SELF_TEST_ID and honours its ignore"

# The lockfiles' own package counts.
frontend_count="$(awk '/^[^#[:space:]].*:$/ { n++ } END { print n + 0 }' "$frontend_lock")"
if ! lexicons_count="$(python3 -I -c '
import json, sys
with open(sys.argv[1], encoding="utf-8") as handle:
    packages = json.load(handle)["packages"]
print(len([name for name in packages if name != ""]))
' "$lexicons_lock")"; then
    say "can't count the packages in $lexicons_lock"
    finish 3
fi

worst=0
# scan_lockfile <name> <config> <lockfile> <count>: scans one lockfile and raises worst.
scan_lockfile() {
    out="$run/$1.out"
    scan "$2" "$3" "$4" "$out"
    cat "$out"
    if [ "$scan_status" != 0 ] && [ "$scan_status" != 1 ]; then
        say "$1: osv-scanner exited $scan_status on $3 ($(osv_kind "$scan_status"))"
        code=3
    elif [ "$count_seen" != 1 ]; then
        say "$1: osv-scanner didn't report the $4 packages $3 holds"
        code=3
    elif [ "$scan_status" = 1 ]; then
        say "$1: advisories no entry in $2 accepts"
        code=1
    else
        say "$1: clean ($4 packages)"
        code=0
    fi
    [ "$code" -gt "$worst" ] && worst="$code"
    return 0
}
scan_lockfile frontend "$frontend_config" "$frontend_lock" "$frontend_count"
scan_lockfile lexicons "$lexicons_config" "$lexicons_lock" "$lexicons_count"

finish "$worst"
