#!/bin/sh
# Tests for scripts/nodes-install.sh against a scratch CARGO_HOME, with curl,
# checksum, tar, cargo and uname stubbed on PATH: no network, no writes outside
# the temporary folder. Run directly, or via `just scripts-test`.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/nodes-install.sh"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT

failures=0
tag=v0.7.1

# stub <name> <body> — a script in $work/stubs that records its call, then runs <body>.
stub() {
    {
        printf '#!/bin/sh\n'
        printf 'echo "%s $*" >>"$STUB_LOG"\n' "$1"
        printf '%s\n' "$2"
    } >"$work/stubs/$1"
    chmod +x "$work/stubs/$1"
}

# nodes_binary <path> <version> — a fake `nodes` that reports <version>.
nodes_binary() {
    mkdir -p "$(dirname "$1")"
    printf '#!/bin/sh\necho "nodes %s"\n' "$2" >"$1"
    chmod +x "$1"
}

# new_world — empty fixture folders and the stubs the install path calls.
new_world() {
    rm -rf "$work/home" "$work/stubs" "$work/path-bin" "$work/log"
    mkdir -p "$work/home" "$work/stubs" "$work/path-bin"
    : >"$work/log"
    stub uname 'case "$1" in -s) echo Linux ;; -m) echo x86_64 ;; esac'
    stub curl 'while [ "$#" -gt 0 ]; do [ "$1" = "-o" ] && : >"$2"; shift; done'
    stub shasum 'exit 0'
    stub sha256sum 'exit 0'
    stub tar "nodes_dir=\"\"; while [ \"\$#\" -gt 0 ]; do [ \"\$1\" = \"-C\" ] && nodes_dir=\"\$2\"; shift; done
printf '#!/bin/sh\necho \"nodes 0.7.1\"\n' >\"\$nodes_dir/nodes\""
    stub cargo 'exit 0'
}

# run_install — the script with CARGO_HOME at the fixture and the stubs, then the
# nodes-on-PATH folder, ahead of the real PATH; output in $work/out, status in $got.
run_install() {
    set +e
    STUB_LOG="$work/log" CARGO_HOME="$work/home" PATH="$work/stubs:$work/path-bin:$PATH" \
        sh "$under_test" "$tag" >"$work/out" 2>&1
    got=$?
    set -e
}

# check <name> <condition command…> — ok/FAIL by the command's status.
check() {
    name="$1"
    shift
    if "$@"; then
        echo "ok   $name"
    else
        echo "FAIL $name"
        awk '{ print "  out: " $0 }' "$work/out"
        awk '{ print "  log: " $0 }' "$work/log"
        failures=$((failures + 1))
    fi
}

exit_zero() { [ "$got" -eq 0 ]; }
out_has() { grep -qF "$1" "$work/out"; }
log_has() { grep -qE "$1" "$work/log"; }
log_empty() { [ ! -s "$work/log" ]; }
installed_in_scratch() {
    [ -x "$work/home/bin/nodes" ] && [ "$("$work/home/bin/nodes" --version)" = "nodes 0.7.1" ]
}

new_world
nodes_binary "$work/home/bin/nodes" 0.7.1
nodes_binary "$work/path-bin/nodes" 0.6.0
run_install
check "the pin in the bin folder is already installed" exit_zero
check "the pin in the bin folder says so" out_has "already installed"
check "the pin in the bin folder fetches nothing" log_empty

new_world
nodes_binary "$work/path-bin/nodes" 0.6.0
run_install
check "another version on PATH only installs: exit 0" exit_zero
check "another version on PATH only does not say installed already" sh -c '! grep -qF "already installed" "$1"' _ "$work/out"
check "another version on PATH only fetches the archive" log_has '^curl .*nodes-v0\.7\.1-x86_64-unknown-linux-gnu\.tar\.gz'
check "another version on PATH only unpacks it into the bin folder" log_has "^tar .*-C $work/home/bin nodes"
check "another version on PATH only installs the pin into the bin folder" installed_in_scratch

new_world
nodes_binary "$work/path-bin/nodes" 0.7.1
run_install
check "the pin on PATH only still installs into the bin folder" installed_in_scratch
check "the pin on PATH only fetches the archive" log_has '^curl '

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all nodes-install tests passed"
