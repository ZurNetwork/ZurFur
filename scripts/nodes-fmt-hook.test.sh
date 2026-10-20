#!/bin/sh
# Tests for scripts/nodes-fmt-hook.sh against fixture trees, with a stub `nodes`
# first on PATH that logs each call's folder and arguments. Run directly, or via
# `just scripts-test`. POSIX sh + awk only — no `sed`.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
hook="$script_dir/nodes-fmt-hook.sh"
sh_bin="$(command -v sh)"
work="$(mktemp -d)"
cleanup() { rm -rf "$work"; }
trap cleanup EXIT
work="$(cd "$work" && pwd -P)"

failures=0

# The stub: logs "<folder>|<arguments>" to $STUB_LOG, prints $STUB_VERSION for
# --version, and fails `fmt` with $STUB_FMT_FAIL on stderr when it is set
# (otherwise prints $STUB_FMT_OUT).
mkdir -p "$work/bin"
cat >"$work/bin/nodes" <<'EOF'
#!/bin/sh
printf '%s|%s\n' "$(pwd -P)" "$*" >>"$STUB_LOG"
case "${1:-}" in
    --version) printf '%s\n' "${STUB_VERSION:-nodes 0.7.1}" ;;
    fmt)
        if [ -n "${STUB_FMT_FAIL:-}" ]; then
            printf '%s\n' "$STUB_FMT_FAIL" >&2
            exit 1
        fi
        [ -z "${STUB_FMT_OUT:-}" ] || printf '%s\n' "$STUB_FMT_OUT"
        ;;
esac
EOF
chmod +x "$work/bin/nodes"

# node <file> <path> — a minimal node file declaring <path>.
node() {
    mkdir -p "${1%/*}"
    printf '{\n  "path": "%s",\n  "short": "fixture"\n}\n' "$2" >"$1"
}

# current_tree <dir> — a current tree: a root NODE.inf.json, the NODE.refs.json
# and a nested node in sub/.
current_tree() {
    node "$1/NODE.inf.json" .
    printf '{}\n' >"$1/NODE.refs.json"
    node "$1/sub/NODE.inf.json" sub
}

A="$work/a"
B="$work/b"
C="$work/no-refs"
R="$work/refs-only"
L="$work/legacy"
S="$work/stray"
# A legacy tree nested in a current one: the walk up from it must stop at its
# own root NODE.json, never reach the current tree's root.
N="$work/nested"
NL="$N/legacy"
current_tree "$A"
current_tree "$B"
current_tree "$S"
node "$S/NODE.json" .
node "$C/NODE.inf.json" .
node "$C/sub/NODE.inf.json" sub
mkdir -p "$R"
printf '{}\n' >"$R/NODE.refs.json"
node "$R/sub/NODE.inf.json" sub
ln -s "$A" "$work/a-link"
node "$L/NODE.json" .
node "$L/sub/NODE.json" sub
node "$L/sub/NODE.inf.json" sub
current_tree "$N"
node "$NL/NODE.json" .
node "$NL/sub/NODE.json" sub
node "$NL/sub/NODE.inf.json" sub
node "$work/loose/NODE.inf.json" loose
mkdir -p "$A/linked"
ln -s "$A/sub/NODE.inf.json" "$A/linked/NODE.inf.json"
printf 'readme\n' >"$A/README.md"
# Folders whose names hold a control character, each with a node file a hook
# that let the name through would format.
tab="$(printf '\t')"
cr="$(printf '\r')"
nl="$(printf '\nx')"
nl="${nl%x}"
for c in "$tab" "$cr" "$nl"; do
    node "$A/sub$c/NODE.inf.json" sub
done
# Without jq the hook cannot decode `\t`: a folder named with those two
# characters is what a hook reading the escape as text would format.
node "$A/sub\\t/NODE.inf.json" sub
# Folders named with a C1 control (U+0080, U+0085, U+009F) or a line or
# paragraph separator (U+2028, U+2029), raw UTF-8; and two that share a lead
# byte with those but are ordinary text (U+00A9, U+2027), which are formatted.
u0080="$(printf '\302\200')"
u0085="$(printf '\302\205')"
u009f="$(printf '\302\237')"
u2028="$(printf '\342\200\250')"
u2029="$(printf '\342\200\251')"
u00a9="$(printf '\302\251')"
u2027="$(printf '\342\200\247')"
for c in "$u0080" "$u0085" "$u009f" "$u2028" "$u2029" "$u00a9" "$u2027"; do
    node "$A/sub$c/NODE.inf.json" sub
done
node "$A/XNODE.inf.json" .

# A PATH without any `nodes`, and a bare one without jq.
nobin_path=""
old_ifs="$IFS"
set -f
IFS=:
for d in $PATH; do
    if [ -n "$d" ] && [ ! -e "$d/nodes" ]; then
        nobin_path="${nobin_path:+$nobin_path:}$d"
    fi
done
IFS="$old_ifs"
set +f
mkdir -p "$work/nojq"
for tool in cat awk grep; do
    ln -s "$(command -v "$tool")" "$work/nojq/$tool"
done
ln -s "$work/bin/nodes" "$work/nojq/nodes"

# json <file_path JSON string body> — a Write tool call.
json() {
    printf '{"tool_name":"Write","tool_input":{"file_path":"%s","content":"x"}}' "$1"
}

# run <cwd> <stdin> [PATH] — runs the hook from <cwd> as a session in A would;
# STUB_VERSION / STUB_FMT_FAIL / STUB_FMT_OUT come from the variables of those
# names in lower case.
stub_version="nodes 0.7.1"
stub_fmt_fail=""
stub_fmt_out=""
run() {
    : >"$work/calls"
    set +e
    (
        cd "$1" &&
            printf '%s' "$2" |
            CLAUDE_PROJECT_DIR="$A" PATH="${3:-$work/bin:$PATH}" STUB_LOG="$work/calls" \
                STUB_VERSION="$stub_version" STUB_FMT_FAIL="$stub_fmt_fail" STUB_FMT_OUT="$stub_fmt_out" \
                "$sh_bin" "$hook" >"$work/out" 2>"$work/err"
    )
    got=$?
    set -e
    stub_version="nodes 0.7.1"
    stub_fmt_fail=""
    stub_fmt_out=""
}

# Checks over the last run.
exit_is() { [ "$got" -eq "$1" ]; }
no_calls() { [ ! -s "$work/calls" ]; }
no_fmt() { ! grep -q '|fmt' "$work/calls"; }
fmt_calls_are() { [ "$(grep -v '|--version$' "$work/calls" || true)" = "$1" ]; }
out_has() { grep -qF -- "$1" "$work/out"; }
err_has() { grep -qF -- "$1" "$work/err"; }
out_empty() { [ ! -s "$work/out" ]; }
err_empty() { [ ! -s "$work/err" ]; }
no_migrate() { ! grep -qi 'migrate' "$work/out" "$work/err"; }
no_echo() { ! grep -qF -- "$A" "$work/out" "$work/err"; }

# expect <name> <check>… — every check holds for the last run.
expect() {
    name="$1"
    shift
    bad=""
    for check in "$@"; do
        eval "$check" || bad="$bad [$check]"
    done
    if [ -z "$bad" ]; then
        echo "ok   $name"
    else
        echo "FAIL $name:$bad (exit $got)"
        awk '{ print "  calls: " $0 }' "$work/calls"
        awk '{ print "  out:   " $0 }' "$work/out"
        awk '{ print "  err:   " $0 }' "$work/err"
        failures=$((failures + 1))
    fi
}

# --- a current tree ---
stub_fmt_out="fmt: sub/NODE.inf.json"
run "$A" "$(json "$A/sub/NODE.inf.json")"
expect "a nested NODE.inf.json is formatted from its own folder" 'exit_is 0' \
    "fmt_calls_are '$A/sub|fmt $A/sub/NODE.inf.json'" 'out_has "fmt: sub/NODE.inf.json"'

run "$A" "$(json "$A/NODE.inf.json")"
expect "the root NODE.inf.json is formatted from the root" 'exit_is 0' \
    "fmt_calls_are '$A|fmt $A/NODE.inf.json'"

run "$A" "$(json "sub/NODE.inf.json")"
expect "a relative path resolves against the current folder" 'exit_is 0' \
    "fmt_calls_are '$A/sub|fmt $A/sub/NODE.inf.json'"

run "$A/sub" "$(json "NODE.inf.json")"
expect "a bare file name resolves against the current folder" 'exit_is 0' \
    "fmt_calls_are '$A/sub|fmt $A/sub/NODE.inf.json'"

run "$A" "$(json "$work/a-link/sub/NODE.inf.json")"
expect "a path through a linked folder is formatted by its physical path" 'exit_is 0' \
    "fmt_calls_are '$A/sub|fmt $A/sub/NODE.inf.json'"

run "$A" "$(json "$C/sub/NODE.inf.json")"
expect "a tree without NODE.refs.json is found by its root node" 'exit_is 0' \
    "fmt_calls_are '$C/sub|fmt $C/sub/NODE.inf.json'"

run "$A" "$(json "$R/sub/NODE.inf.json")"
expect "a tree without a root node is found by its NODE.refs.json" 'exit_is 0' \
    "fmt_calls_are '$R/sub|fmt $R/sub/NODE.inf.json'"

run "$A" "$(json "$B/sub/NODE.inf.json")"
expect "a NODE.inf.json in another checkout is formatted in its own tree" 'exit_is 0' \
    "fmt_calls_are '$B/sub|fmt $B/sub/NODE.inf.json'"

run "$B/sub" "$(json "$A/NODE.refs.json")"
expect "NODE.refs.json gets a bare fmt at its root" 'exit_is 0' "fmt_calls_are '$A|fmt'"

node "$A/sub/NODE.json" sub
run "$A" "$(json "$A/sub/NODE.json")"
expect "a nested NODE.json in a current tree is refused" 'exit_is 2' 'no_calls' \
    'err_has NODE.inf.json' 'err_has "delete this NODE.json"' \
    'err_has "write NODE.inf.json instead"' 'no_migrate'
rm "$A/sub/NODE.json"

node "$A/NODE.json" .
run "$A" "$(json "$A/NODE.json")"
expect "a root NODE.json in a current tree is refused" 'exit_is 2' 'no_calls' \
    'err_has NODE.inf.json' 'err_has "delete this NODE.json"' \
    'err_has "write NODE.inf.json instead"' 'no_migrate'
rm "$A/NODE.json"

# --- a tree from before 0.7 ---
run "$A" "$(json "$L/sub/NODE.json")"
expect "a NODE.json in a legacy tree is left alone, silently" 'exit_is 0' 'no_calls' \
    'out_empty' 'err_empty'
run "$A" "$(json "$L/sub/NODE.inf.json")"
expect "a NODE.inf.json in a legacy tree is left alone, silently" 'exit_is 0' 'no_calls' \
    'out_empty' 'err_empty'
run "$A" "$(json "$L/NODE.json")"
expect "a legacy root NODE.json is left alone, silently" 'exit_is 0' 'no_calls' \
    'out_empty' 'err_empty'
run "$A" "$(json "$NL/NODE.json")"
expect "a legacy tree nested in a current one: its root NODE.json is left alone" \
    'exit_is 0' 'no_calls' 'out_empty' 'err_empty'
run "$A" "$(json "$NL/sub/NODE.json")"
expect "a legacy tree nested in a current one: a nested NODE.json is left alone" \
    'exit_is 0' 'no_calls' 'out_empty' 'err_empty'
run "$A" "$(json "$NL/sub/NODE.inf.json")"
expect "a legacy tree nested in a current one: a NODE.inf.json is left alone" \
    'exit_is 0' 'no_calls' 'out_empty' 'err_empty'

# --- a stray root NODE.json beside a current tree's ---
run "$A" "$(json "$S/sub/NODE.inf.json")"
expect "a stray root NODE.json blocks a NODE.inf.json write" 'exit_is 2' 'no_calls' \
    "err_has '$S/NODE.json'" 'no_migrate'
run "$A" "$(json "$S/NODE.refs.json")"
expect "a stray root NODE.json blocks a NODE.refs.json write" 'exit_is 2' 'no_calls' \
    "err_has '$S/NODE.json'" 'no_migrate'

# --- other files ---
run "$A" "$(json "$A/README.md")"
expect "README.md gets no call" 'exit_is 0' 'no_calls' 'out_empty' 'err_empty'
run "$A" "$(json "$A/XNODE.inf.json")"
expect "XNODE.inf.json gets no call" 'exit_is 0' 'no_calls' 'out_empty' 'err_empty'

# --- nodes refuses the file ---
stub_fmt_fail="nodes: $A/sub/NODE.inf.json: missing field \`short\` at line 3 column 1"
run "$A" "$(json "$A/sub/NODE.inf.json")"
expect "a schema error is fed back with exit 2" 'exit_is 2' \
    'err_has "missing field \`short\` at line 3 column 1"'

stub_fmt_fail="nodes: $A/sub/NODE.inf.json: unknown field \`refs\` at line 9 column 9 — an older release wrote it; \`nodes migrate\` brings it up to date (refs move into NODE.refs.json)"
run "$A" "$(json "$A/sub/NODE.inf.json")"
expect "a needs-migrating error names add-ref, never migrate" 'exit_is 2' 'no_migrate' \
    'err_has "nodes add-ref"' 'err_has "unknown field \`refs\`"'

stub_fmt_fail="nodes: $A/sub holds both NODE.json and NODE.inf.json, and they differ; delete the one that is out of date, then migrate"
run "$A" "$(json "$A/sub/NODE.inf.json")"
expect "any other nodes message naming migrate is replaced" 'exit_is 2' 'no_migrate' \
    'err_has "nodes-fmt-hook:"'

# --- the binary ---
if PATH="$nobin_path" command -v nodes >/dev/null 2>&1; then
    echo "FAIL setup: a PATH without nodes still finds $(PATH="$nobin_path" command -v nodes)"
    failures=$((failures + 1))
else
    run "$A" "$(json "$A/sub/NODE.inf.json")" "$nobin_path"
    expect "no binary: exit 0 and a hint" 'exit_is 0' 'out_has "nodes not installed"' \
        'out_has "just nodes-install"'
fi

stub_version="nodes 0.6.0"
run "$A" "$(json "$A/sub/NODE.inf.json")"
expect "a nodes older than 0.7: exit 0, a hint, no fmt" 'exit_is 0' 'no_fmt' \
    'out_has "nodes 0.6.0 is older than this tree"' 'out_has "just nodes-install"' 'no_migrate'

# --- refused without a call ---
run "$A" "$(json "$work/loose/NODE.inf.json")"
expect "a node file outside any tree gets no call" 'exit_is 0' 'no_calls' 'out_empty' 'err_empty'

run "$A" "$(json "$A/sub/NODE.inf.json\\n")"
expect "a trailing newline gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
run "$A" "$(json "$A/sub\\n/NODE.inf.json")"
expect "a newline in a folder name gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
run "$A" "$(json "$A/sub\\r/NODE.inf.json")"
expect "a carriage return gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
run "$A" "$(json "$A/sub\\t/NODE.inf.json")"
expect "a tab gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
run "$A" "$(json "$A/sub/NODE.inf.json\\u0000")"
expect "a NUL gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
run "$A" "$(json "$A/sub\\u0085/NODE.inf.json")"
expect "an escaped U+0085 gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
run "$A" "$(json "$A/sub$u0085/NODE.inf.json")"
expect "a raw U+0085 gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
run "$A" "$(json "$A/sub$u0080/NODE.inf.json")"
expect "a raw U+0080 gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
run "$A" "$(json "$A/sub$u009f/NODE.inf.json")"
expect "a raw U+009F gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
run "$A" "$(json "$A/sub$u2028/NODE.inf.json")"
expect "a raw U+2028 gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
run "$A" "$(json "$A/sub$u2029/NODE.inf.json")"
expect "a raw U+2029 gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
run "$A" "$(json "$A/sub$u00a9/NODE.inf.json")"
expect "a U+00A9 in a folder name is formatted" 'exit_is 0' \
    "fmt_calls_are '$A/sub$u00a9|fmt $A/sub$u00a9/NODE.inf.json'"
run "$A" "$(json "$A/sub$u2027/NODE.inf.json")"
expect "a U+2027 in a folder name is formatted" 'exit_is 0' \
    "fmt_calls_are '$A/sub$u2027|fmt $A/sub$u2027/NODE.inf.json'"

run "$A" "$(json "$A/linked/NODE.inf.json")"
expect "a symbolic link gets no call and exit 2" 'exit_is 2' 'no_calls' 'err_has "symbolic link"'

# --- without jq ---
if PATH="$work/nojq" command -v jq >/dev/null 2>&1; then
    echo "FAIL setup: the bare PATH still finds jq"
    failures=$((failures + 1))
else
    run "$A" "$(json "$B/sub/NODE.inf.json")" "$work/nojq"
    expect "without jq: a NODE.inf.json is formatted in its own tree" 'exit_is 0' \
        "fmt_calls_are '$B/sub|fmt $B/sub/NODE.inf.json'"
    run "$A" "$(json "$A/NODE.refs.json")" "$work/nojq"
    expect "without jq: NODE.refs.json gets a bare fmt at its root" 'exit_is 0' "fmt_calls_are '$A|fmt'"
    run "$A" "$(json "$C/sub/NODE.inf.json")" "$work/nojq"
    expect "without jq: a tree without NODE.refs.json is found by its root node" 'exit_is 0' \
        "fmt_calls_are '$C/sub|fmt $C/sub/NODE.inf.json'"
    run "$A" "$(json "$L/sub/NODE.inf.json")" "$work/nojq"
    expect "without jq: a legacy tree is left alone, silently" 'exit_is 0' 'no_calls' \
        'out_empty' 'err_empty'
    run "$A" "$(json "$NL/NODE.json")" "$work/nojq"
    expect "without jq: a nested legacy tree's root NODE.json is left alone" \
        'exit_is 0' 'no_calls' 'out_empty' 'err_empty'
    run "$A" "$(json "$NL/sub/NODE.json")" "$work/nojq"
    expect "without jq: a nested legacy tree's nested NODE.json is left alone" \
        'exit_is 0' 'no_calls' 'out_empty' 'err_empty'
    run "$A" "$(json "$NL/sub/NODE.inf.json")" "$work/nojq"
    expect "without jq: a nested legacy tree's NODE.inf.json is left alone" \
        'exit_is 0' 'no_calls' 'out_empty' 'err_empty'
    run "$A" "$(json "$A/sub\\t/NODE.inf.json")" "$work/nojq"
    expect "without jq: a control character gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
    for c in u0080 u0085 u009f u2028 u2029; do
        eval "raw=\$$c"
        run "$A" "$(json "$A/sub$raw/NODE.inf.json")" "$work/nojq"
        expect "without jq: a raw U+${c#u} gets no call and no echo" 'exit_is 0' 'no_calls' 'no_echo'
    done
    run "$A" "$(json "$A/sub$u00a9/NODE.inf.json")" "$work/nojq"
    expect "without jq: a U+00A9 in a folder name is formatted" 'exit_is 0' \
        "fmt_calls_are '$A/sub$u00a9|fmt $A/sub$u00a9/NODE.inf.json'"
    run "$A" "$(json "$A/sub$u2027/NODE.inf.json")" "$work/nojq"
    expect "without jq: a U+2027 in a folder name is formatted" 'exit_is 0' \
        "fmt_calls_are '$A/sub$u2027|fmt $A/sub$u2027/NODE.inf.json'"
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all nodes-fmt-hook tests passed"
