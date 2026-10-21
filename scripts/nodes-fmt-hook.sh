#!/bin/sh
# Claude Code PostToolUse hook (Write|Edit), wired in .claude/settings.json:
# keeps every node file canonical without anyone remembering to. Reads the tool
# call from stdin and acts only on a NODE.inf.json, a NODE.refs.json or a
# NODE.json. It finds the file's tree the way nodes does, walking up from the
# file's folder to the nearest root (a NODE.refs.json, or a NODE.inf.json or
# NODE.json declaring "path": "."), so a file in another checkout than the
# session's is formatted in its own tree:
#
#   - a tree from before nodes 0.7 (its root holds a NODE.json and neither a
#     NODE.inf.json nor a NODE.refs.json): nothing, silently; nodes writes such
#     a tree only once it has caught up.
#   - NODE.inf.json: `nodes fmt <file>` from the file's folder.
#   - NODE.refs.json: a bare `nodes fmt` at the root (fmt takes node files
#     alone as arguments).
#   - NODE.json: refused with exit 2; node files are NODE.inf.json since 0.7.
#   - a NODE.json at the root of a current tree: every node-file write there
#     is refused with exit 2; nodes reads that tree as one from before 0.7 and
#     writes nothing in it.
#
# A schema error (the model wrote a file nodes refuses) is fed back to the
# model via exit 2 + stderr, so it fixes the file at once. Refused without a
# call: a path holding a control character (C0, DEL or C1) or a line or
# paragraph separator (U+2028, U+2029) (silently, never echoed), a symbolic
# link (exit 2), a file outside any tree (silently).
#
# Fails SOFT: without the `nodes` binary, or with one older than 0.7, it prints
# a one-line hint and exits 0; the hard backstop is `just nodes-check` in the
# gate and CI.
set -eu

input="$(cat)"

# The path, or "!" when it holds a control character, U+2028 or U+2029 (or
# cannot be read safely): a refused path is never echoed back.
if command -v jq >/dev/null 2>&1; then
    file="$(printf '%s' "$input" | jq -j '
        (.tool_input.file_path // "") as $f
        | if ($f | explode | any(. < 32 or (. >= 127 and . <= 159) or . == 8232 or . == 8233))
          then "!" else $f end
    ' 2>/dev/null)" || exit 0
else
    # Without jq a JSON escape cannot be decoded: a value holding a backslash
    # (every C0 control in valid JSON is escaped) is refused, and so are the
    # raw UTF-8 bytes of C1 controls (C2 80-9F), U+2028 and U+2029 (E2 80 A8-A9),
    # matched byte by byte in the C locale.
    file="$(printf '%s' "$input" | LC_ALL=C awk '
        {
            if (!match($0, /"file_path"[[:space:]]*:[[:space:]]*"/)) next
            rest = substr($0, RSTART + RLENGTH)
            end_at = index(rest, "\"")
            if (end_at == 0) { printf "!"; exit }
            value = substr(rest, 1, end_at - 1)
            if (value ~ /[\\[:cntrl:]]|\302[\200-\237]|\342\200[\250\251]/) printf "!"
            else printf "%s", value
            exit
        }')"
fi
case "$file" in
    '!' | '') exit 0 ;;
esac

case "$file" in
    /*) ;;
    *) file="$PWD/$file" ;;
esac

case "$file" in
    */NODE.inf.json | NODE.inf.json) kind=node ;;
    */NODE.refs.json | NODE.refs.json) kind=refs ;;
    */NODE.json | NODE.json) kind=legacy ;;
    *) exit 0 ;;
esac

if [ -L "$file" ]; then
    printf 'nodes-fmt-hook: %s is a symbolic link; node files are written in place, never through a link\n' "$file" >&2
    exit 2
fi
[ -f "$file" ] || exit 0

base="${file##*/}"
parent="${file%/*}"
[ -n "$parent" ] || parent=/
# The folder as nodes sees it from there: the physical path.
dir="$(cd "$parent" 2>/dev/null && pwd -P)" || exit 0
file="${dir%/}/$base"

# declares_root <node file> — the file declares "path": ".".
declares_root() {
    [ -f "$1" ] || return 1
    if command -v jq >/dev/null 2>&1; then
        jq -e 'type == "object" and .path == "."' "$1" >/dev/null 2>&1
    else
        grep -Eq '^[[:space:]]*"path"[[:space:]]*:[[:space:]]*"\."[[:space:]]*,?[[:space:]]*$' "$1"
    fi
}

# holds_root <dir> — the folder roots a tree.
holds_root() {
    [ -f "$1/NODE.refs.json" ] || declares_root "$1/NODE.inf.json" || declares_root "$1/NODE.json"
}

root="$dir"
until holds_root "$root"; do
    [ "$root" != / ] || exit 0
    root="${root%/*}"
    [ -n "$root" ] || root=/
done
root_file() { printf '%s/%s' "${root%/}" "$1"; }

if [ ! -f "$(root_file NODE.inf.json)" ] && [ ! -f "$(root_file NODE.refs.json)" ]; then
    exit 0
fi

if [ "$kind" = legacy ]; then
    printf 'nodes-fmt-hook: %s: node files are NODE.inf.json since nodes 0.7; delete this NODE.json and write NODE.inf.json instead\n' "$file" >&2
    exit 2
fi

if [ -f "$(root_file NODE.json)" ]; then
    printf 'nodes-fmt-hook: %s sits beside this tree'"'"'s NODE.inf.json files, so nodes reads the whole tree as older than 0.7 and writes nothing in it; %s is left unformatted\n' "$(root_file NODE.json)" "$file" >&2
    exit 2
fi

if ! command -v nodes >/dev/null 2>&1; then
    echo "nodes not installed — run \`just setup\` (or \`just nodes-install\`); $file left unformatted"
    exit 0
fi

have="$(nodes --version 2>/dev/null | awk 'NR == 1 { print $2 }')" || have=""
current="$(printf '%s\n' "$have" | awk -F. '
    $1 ~ /^[0-9]+$/ && $2 ~ /^[0-9]+/ { print ($1 > 0 || $2 + 0 >= 7) ? "yes" : "no"; exit }
    { print "no" }')"
if [ "$current" != yes ]; then
    echo "nodes ${have:-of an unknown version} is older than this tree (node files are NODE.inf.json since nodes 0.7); install the pin with \`just nodes-install\`; $file left unformatted"
    exit 0
fi

if [ "$kind" = refs ]; then
    status=0
    output="$(cd "$root" && nodes fmt 2>&1)" || status=$?
else
    status=0
    output="$(cd "$dir" && nodes fmt "$file" 2>&1)" || status=$?
fi

if [ "$status" -ne 0 ]; then
    # nodes' own advice for an older file, or a tree-wide step, is not this
    # file's to take: it is replaced by what the model should write instead.
    printf '%s\n' "$output" | awk '
        index($0, " — an older release wrote it") {
            print substr($0, 1, index($0, " — an older release wrote it") - 1) \
                " — the shape nodes wrote before 0.7: a NODE.inf.json holds no refs (they live in the tree'"'"'s NODE.refs.json; add one with `nodes add-ref <path> <page> \"<title>\" \"<governs>\"`), and categories are `category:` tags"
            next
        }
        /(^|[^[:alnum:]_.\/-])[Mm][Ii][Gg][Rr][Aa][Tt][Ee]([^[:alnum:]_.\/-]|$)/ {
            print "nodes-fmt-hook: nodes refused to format this file for a reason that concerns the whole tree; run `just nodes-check`"
            next
        }
        { print }' >&2
    exit 2
fi
[ -z "$output" ] || printf '%s\n' "$output"
