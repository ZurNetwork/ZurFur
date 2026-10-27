#!/bin/sh
# Pins the gallery's screenshot image and its wiring: the Justfile's one
# PLAYWRIGHT_IMAGE line, its tag version against the @playwright/test that
# frontend/yarn.lock locks, the `gallery-shots` and `gallery-shots-run` recipes
# line for line, and the container flags on gallery-shots.sh's run line. Each
# pin's failure is proved on a doctored copy in a temporary folder. Run
# directly, or via `just scripts-test`; `--root <dir>` checks another tree.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
self="$script_dir/$(basename "$0")"
image='mcr.microsoft.com/playwright:v1.61.1-noble@sha256:cf0daee9b994042e011bc29f20cdff1a9f682a039b43fcd738f7d8a9d3bcd9d6'
if [ "${1-}" = --root ] && [ -n "${2-}" ]; then
    root="$2"
    prove=0
else
    root="$(cd "$script_dir/.." && pwd)"
    prove=1
fi
justfile="$root/Justfile"
lock="$root/frontend/yarn.lock"
script="$root/scripts/gallery-shots.sh"
failures=0

ok() { echo "ok   $1"; }
fail() {
    echo "FAIL $1: $2"
    failures=$((failures + 1))
}

# --- the image pin ----------------------------------------------------------------

assignments="$(grep -cE '^(export[[:space:]]+)?PLAYWRIGHT_IMAGE[[:space:]]*:?=' "$justfile" || true)"
if [ "$assignments" = 1 ]; then
    ok "the Justfile sets PLAYWRIGHT_IMAGE once"
else
    fail "the Justfile sets PLAYWRIGHT_IMAGE once" "$assignments assignments"
fi
if grep -qxF -- "export PLAYWRIGHT_IMAGE := \"$image\"" "$justfile"; then
    ok "PLAYWRIGHT_IMAGE is exactly the pinned image, exported"
else
    fail "PLAYWRIGHT_IMAGE is exactly the pinned image, exported" "no line 'export PLAYWRIGHT_IMAGE := \"$image\"'"
fi

# The tag version of the Justfile's pin, and the version(s) the lock gives @playwright/test.
tag_version="$(awk '
    /^export PLAYWRIGHT_IMAGE := "mcr\.microsoft\.com\/playwright:v/ {
        line = $0
        sub(/^[^:]*:= "mcr\.microsoft\.com\/playwright:v/, "", line)
        sub(/-.*$/, "", line)
        print line
        exit
    }
' "$justfile")"
locked="$(awk '
    /^[[:space:]]*$/ { inside = 0; next }
    /^[^[:space:]#]/ { inside = ($0 ~ /^"@playwright\/test@/); next }
    inside && /^[[:space:]]+version[[:space:]]/ { version = $2; gsub(/"/, "", version); print version }
' "$lock" | sort -u)"
if [ -n "$tag_version" ] && [ "$tag_version" = "$locked" ]; then
    ok "PLAYWRIGHT_IMAGE's tag version is the locked @playwright/test's ($locked)"
else
    fail "PLAYWRIGHT_IMAGE's tag version is the locked @playwright/test's" \
        "tag '$tag_version', locked '$(printf '%s' "$locked" | tr '\n' ' ')'"
fi

# --- the recipes ------------------------------------------------------------------

# recipe <name>: the header of the recipe called <name> and its body, up to the
# first line that isn't indented.
recipe() {
    awk -v name="$1" '
        inside && /^[[:space:]]+[^[:space:]]/ { print; next }
        inside { exit }
        index($0, name " ") == 1 || index($0, name ":") == 1 { inside = 1; print }
    ' "$justfile"
}
# expect_recipe <name> <lines…>: the recipe is exactly <lines>, and its name heads one recipe.
expect_recipe() {
    name="$1"
    shift
    want="$(printf '%s\n' "$@")"
    got="$(recipe "$name")"
    headers="$(awk -v name="$name" 'index($0, name " ") == 1 || index($0, name ":") == 1 { n++ } END { print n + 0 }' "$justfile")"
    if [ "$got" = "$want" ] && [ "$headers" = 1 ]; then
        ok "the $name recipe is exactly its lines"
    else
        fail "the $name recipe is exactly its lines" "$headers header(s); got:
$got"
    fi
}
expect_recipe gallery-shots \
    'gallery-shots *ARGS:' \
    '    just frontend-install' \
    '    just gallery-build' \
    '    just gallery-shots-run "$@"'
expect_recipe gallery-shots-run \
    'gallery-shots-run *ARGS:' \
    '    bash scripts/gallery-shots.sh run "$@"'

# --- the container flags ----------------------------------------------------------

run_lines="$(grep -c '^run_flags=(run ' "$script" || true)"
run_line="$(grep '^run_flags=(run ' "$script" | tr '()' '  ' || true)"
if [ "$run_lines" = 1 ]; then
    ok "gallery-shots.sh has one run line"
else
    fail "gallery-shots.sh has one run line" "$run_lines lines start 'run_flags=(run '"
fi
for flag in "--network none" "--read-only" "--cap-drop ALL" "--security-opt no-new-privileges" \
    "--init" "--tmpfs /tmp:rw,nosuid,size=2g"; do
    case " $run_line " in
        *" $flag "*) ok "the run line names $flag" ;;
        *) fail "the run line names $flag" "line: $run_line" ;;
    esac
done

# --- each pin fails on a doctored copy --------------------------------------------

if [ "$prove" = 1 ]; then
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT

    # doctor <file> <line prefix> <from> <to>: on the first line starting with
    # <line prefix> that holds <from>, the first <from> becomes <to>.
    doctor() {
        prefix="$2" from="$3" to="$4" awk '
            BEGIN { prefix = ENVIRON["prefix"]; from = ENVIRON["from"]; to = ENVIRON["to"] }
            !done && index($0, prefix) == 1 && (at = index($0, from)) {
                $0 = substr($0, 1, at - 1) to substr($0, at + length(from))
                done = 1
            }
            { print }
        ' "$1" >"$1.new"
        mv "$1.new" "$1"
    }
    # proves <pin name> <file> <line prefix> <from> <to>: on a fresh copy so
    # doctored, the check fails naming that pin.
    proves() {
        copy="$work/copy"
        rm -rf "$copy"
        mkdir -p "$copy/frontend" "$copy/scripts"
        cp "$justfile" "$copy/Justfile"
        cp "$lock" "$copy/frontend/yarn.lock"
        cp "$script" "$copy/scripts/gallery-shots.sh"
        before="$(cat "$copy/$2")"
        doctor "$copy/$2" "$3" "$4" "$5"
        if [ "$(cat "$copy/$2")" = "$before" ]; then
            fail "doctoring $2 for '$1'" "'$4' not found on a line starting '$3'"
            return
        fi
        case_name="a doctored copy fails '$1' (in $2, '$4' changed)"
        if out="$(sh "$self" --root "$copy")"; then
            fail "$case_name" "the check passed"
        elif printf '%s\n' "$out" | grep -qF -- "FAIL $1"; then
            ok "$case_name"
        else
            fail "$case_name" "$out"
        fi
    }
    pin_line='export PLAYWRIGHT_IMAGE := '
    proves "PLAYWRIGHT_IMAGE is exactly the pinned image, exported" Justfile "$pin_line" cf0daee9 cf0daee8
    proves "PLAYWRIGHT_IMAGE is exactly the pinned image, exported" Justfile "$pin_line" "export " ""
    proves "the Justfile sets PLAYWRIGHT_IMAGE once" Justfile "export JUST_VERSION" \
        'export JUST_VERSION := "1.49.0"' "PLAYWRIGHT_IMAGE := \"x\""
    proves "PLAYWRIGHT_IMAGE's tag version is the locked @playwright/test's" Justfile "$pin_line" ":v1.61.1-" ":v1.61.2-"
    proves "PLAYWRIGHT_IMAGE's tag version is the locked @playwright/test's" frontend/yarn.lock \
        '"@playwright/test@' '"@playwright/test@' '"@playwright/test@1.0.0":
  version "1.0.0"

"@playwright/test@'
    proves "the gallery-shots recipe is exactly its lines" Justfile "    just gallery-build" "just gallery-build" "true"
    proves "the gallery-shots recipe is exactly its lines" Justfile "    just gallery-shots-run" '"$@"' '"$@"
    echo extra'
    proves "the gallery-shots recipe is exactly its lines" Justfile "gallery-shots *ARGS:" "*ARGS:" "*ARGS: gallery-build"
    proves "the gallery-shots recipe is exactly its lines" Justfile "gallery-build:" "gallery-build:" "gallery-shots:
    true

gallery-build:"
    proves "the gallery-shots-run recipe is exactly its lines" Justfile "    bash scripts/gallery-shots.sh" "run " "run --update "
    proves "the gallery-shots-run recipe is exactly its lines" Justfile "gallery-shots-run *ARGS:" " *ARGS" ""
    proves "gallery-shots.sh has one run line" scripts/gallery-shots.sh "run_flags=(run " "run_flags=(run " "run_flags=(run --rm)
run_flags=(run "
    for flag in "--network none" "--read-only" "--cap-drop ALL" "--security-opt no-new-privileges" \
        "--init" "--tmpfs /tmp:rw,nosuid,size=2g"; do
        proves "the run line names $flag" scripts/gallery-shots.sh "run_flags=(run " " $flag" ""
    done
    proves "the run line names --network none" scripts/gallery-shots.sh "run_flags=(run " "--network none" "--network host"
    proves "the run line names --init" scripts/gallery-shots.sh "run_flags=(run " "--init)" "--initx)"
fi

if [ "$failures" -gt 0 ]; then
    echo "$failures gallery image pin(s) missing"
    exit 1
fi
echo "all gallery image pins hold"
