#!/bin/sh
# Tests for scripts/nodes-0-7-edits.sh. Run directly, or via `just scripts-test`.
# Needs jq and a real nodes 0.7 or later on PATH (the script's normalization is
# nodes' own); without them it prints one skip line and passes, as in CI's
# `hooks` job, which has no nodes. The fixtures are built from the script's own
# edit table (`--print-edits`), so no old or new text is kept twice.
set -eu

script_dir="$(cd "$(dirname "$0")" && pwd)"
under_test="$script_dir/nodes-0-7-edits.sh"
sh_bin="$(command -v sh)"
failures=0

if ! command -v jq >/dev/null 2>&1; then
    echo "skip (no jq on PATH): the nodes-0-7-edits tests"
    exit 0
fi
real_nodes="$(command -v nodes 2>/dev/null)" || real_nodes=""
have=""
[ -z "$real_nodes" ] || have="$("$real_nodes" --version 2>/dev/null | awk 'NR == 1 { print $2 }')" || have=""
current="$(printf '%s\n' "$have" | awk -F. '
    $1 ~ /^[0-9]+$/ && $2 ~ /^[0-9]+/ { print ($1 > 0 || $2 + 0 >= 7) ? "yes" : "no"; exit }
    { print "no" }')"
if [ "$current" != yes ]; then
    if [ -z "$real_nodes" ]; then
        echo "skip (no nodes on PATH; these tests need nodes 0.7 or later): the nodes-0-7-edits tests"
    else
        echo "skip (nodes ${have:-of an unknown version} on PATH is older than 0.7): the nodes-0-7-edits tests"
    fi
    exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

table="$("$sh_bin" "$under_test" --print-edits)"
targets="$(printf '%s' "$table" | jq -r '[.[].node] | unique | .[]')"

# A `nodes` first on PATH that logs each `set` call to $NODES_SET_LOG, then
# runs the real binary.
mkdir "$tmp/wrap"
cat >"$tmp/wrap/nodes" <<WRAP
#!/bin/sh
for arg in "\$@"; do
    if [ "\$arg" = set ]; then
        echo "\$*" >>"\$NODES_SET_LOG"
        break
    fi
done
exec "$real_nodes" "\$@"
WRAP
chmod +x "$tmp/wrap/nodes"
wrap_path="$tmp/wrap:$PATH"

mkdir "$tmp/old-nodes"
printf '#!/bin/sh\necho "nodes 0.6.0"\n' >"$tmp/old-nodes/nodes"
chmod +x "$tmp/old-nodes/nodes"

# The fixture's nodes, as JSON lines, built from the table: with $mode "old"
# each target holds the old text (inside a longer element; edits sharing a
# number, like 1a and 1b, share one element), each entry to add is absent and
# each element to remove is there; with "new", what a correct run leaves.
builder='
def text($e): if $mode == "old" then $e.old else $e.new end;
def group: .id | sub("[a-z]$"; "");
def strings($node; $field):
    [.[] | select(.node == $node and .field == $field and (.op == "replace" or .op == "remove"))]
    | group_by(group)
    | map(if .[0].op == "remove" then (if $mode == "old" then [.[0].text] else [] end)
          else ["A lead-in, " + (map(text(.)) | join(", then ")) + ", and the rest."] end)
    | add // []
    | . + ["An unrelated element that stays as it is."];
def entries($node):
    [.[] | select(.node == $node and .field == "fs")
        | if .op == "replace-role" then {name: .entry, node: false, role: ("A lead-in, " + text(.) + ", and the rest.")}
          elif .op == "add-entry" then (if $mode == "old" then empty else {name: .entry, node: false, role: .role} end)
          else empty end]
    + [{name: "unrelated.sh", node: false, role: "an unrelated entry"}];
. as $table
| ([.[].node] | unique) as $targets
| $targets[]
| . as $node
| {path: $node, charted: "2026-10-19", short: "fixture node", is: "A fixture node.", type: "code",
   tags: ["category:source"],
   conventions: ($table | strings($node; "conventions")),
   entry_points: [],
   fs: (($table | entries($node))
        + (if $node == "." then [$targets[] | select(. != ".") | {name: (. + "/"), node: true, role: "a target node"}] else [] end)),
   notes: ($table | strings($node; "notes"))}
'

# build <dir> <mode>: a current tree of the target nodes, formatted by nodes.
build() {
    mkdir -p "$1/scripts"
    cp "$under_test" "$1/scripts/"
    printf '%s' "$table" | jq -c --arg mode "$2" "$builder" >"$tmp/nodes.jsonl"
    while IFS= read -r line; do
        path="$(printf '%s' "$line" | jq -r '.path')"
        mkdir -p "$1/$path"
        printf '%s\n' "$line" >"$1/$path/NODE.inf.json"
    done <"$tmp/nodes.jsonl"
    "$real_nodes" --root "$1" fmt >/dev/null
    "$real_nodes" --root "$1" check >"$tmp/check.out" 2>&1 || {
        echo "FAIL the $2 fixture is no valid tree: $(cat "$tmp/check.out")"
        exit 1
    }
}

# sums <dir>: one checksum line per target node file.
sums() {
    printf '%s\n' "$targets" | while IFS= read -r target; do cksum "$1/$target/NODE.inf.json"; done
}

# run <dir> [arg]: runs the copy of the script in <dir> (PATH is $case_path,
# default the logging wrapper first); sets $out and $got, empties the set log.
run() {
    : >"$tmp/sets"
    out="$(NODES_SET_LOG="$tmp/sets" PATH="${case_path:-$wrap_path}" "$sh_bin" "$1/scripts/nodes-0-7-edits.sh" ${2+"$2"} 2>&1)" &&
        got=0 || got=$?
}

fail() {
    echo "FAIL $1"
    failures=$((failures + 1))
}

# holds <text>...: $out holds every text.
holds() {
    for text in "$@"; do
        case "$out" in
            *"$text"*) ;;
            *) return 1 ;;
        esac
    done
}

# reword <dir> <edit id>: rewords that edit's old text in the fixture.
reword() {
    edit="$(printf '%s' "$table" | jq -c --arg id "$2" '.[] | select(.id == $id)')"
    node="$(printf '%s' "$edit" | jq -r '.node')"
    jq --argjson e "$edit" '
        if $e.op == "replace-role" then .fs |= map(if .name == $e.entry then .role |= (split($e.old) | join("a reworded text")) else . end)
        else .[$e.field] |= map(split($e.old) | join("a reworded text")) end
    ' "$1/$node/NODE.inf.json" >"$tmp/reworded"
    cat "$tmp/reworded" >"$1/$node/NODE.inf.json"
    "$real_nodes" --root "$1" fmt >/dev/null
}

ids="$(printf '%s' "$table" | jq -r '.[].id')"
count="$(printf '%s' "$table" | jq 'length')"

build "$tmp/expected" new

# --- the old tree, checked, applied, checked, applied again ---
r="$tmp/main"
build "$r" old

run "$r" --check
lines="$(printf '%s\n' "$out" | awk '/^nodes-0-7-edits: pending: edit / { n++ } END { print n + 0 }')"
listed=yes
for id in $ids; do holds "pending: edit $id, " || listed=no; done
if [ "$got" = 1 ] && [ "$lines" = "$count" ] && [ "$listed" = yes ] && [ ! -s "$tmp/sets" ]; then
    echo "ok   --check on the old tree exits 1 and lists every edit as pending"
else
    fail "--check on the old tree: expected exit 1 and $count pending lines, got $got and $lines: $out"
fi

before="$(sums "$r")"
run "$r" --check
if [ "$(sums "$r")" = "$before" ]; then
    echo "ok   --check changes no file"
else
    fail "--check changed a file"
fi

run "$r"
same=yes
for node in $targets; do
    if ! jq -S . "$r/$node/NODE.inf.json" >"$tmp/got.json" ||
        ! jq -S . "$tmp/expected/$node/NODE.inf.json" >"$tmp/want.json" ||
        ! cmp -s "$tmp/got.json" "$tmp/want.json"; then
        same=no
        echo "     $node differs from the expected node:"
        diff "$tmp/want.json" "$tmp/got.json" | awk '{ print "     " $0 }' || :
    fi
done
fields="$(printf '%s' "$table" | jq '[.[] | [.node, .field]] | unique | length')"
calls="$(awk 'END { print NR }' "$tmp/sets")"
if [ "$got" = 0 ] && [ "$same" = yes ] && holds "OK, all $count edits applied" && [ "$calls" = "$fields" ]; then
    echo "ok   apply exits 0, makes one set per field, and leaves exactly the expected nodes"
else
    fail "apply: expected exit 0, the expected nodes and $fields set calls, got $got, same=$same and $calls calls: $out"
fi

run "$r" --check
if [ "$got" = 0 ] && holds "OK, all $count edits applied"; then
    echo "ok   --check after apply exits 0"
else
    fail "--check after apply: expected exit 0 and the OK line, got $got: $out"
fi

before="$(sums "$r")"
run "$r"
calls="$(awk 'END { print NR }' "$tmp/sets")"
if [ "$got" = 0 ] && [ "$(sums "$r")" = "$before" ] && [ "$calls" = 0 ]; then
    echo "ok   a second apply changes no byte and makes no set call"
else
    fail "a second apply: expected exit 0, no change and no set call, got $got and $calls call(s): $out"
fi

# --- a reworded target ---
r="$tmp/reworded-tree"
build "$r" old
victim="$(printf '%s' "$table" | jq -r '[.[] | select(.op == "replace")] | last | .id')"
reword "$r" "$victim"
run "$r" --check
if [ "$got" = 1 ] && holds "missing: edit $victim, "; then
    echo "ok   --check names a reworded target as missing"
else
    fail "--check on a reworded target: expected exit 1 naming edit $victim missing, got $got: $out"
fi
before="$(sums "$r")"
run "$r"
if [ "$got" = 1 ] && holds "missing: edit $victim, " "nothing written" && [ "$(sums "$r")" = "$before" ] && [ ! -s "$tmp/sets" ]; then
    echo "ok   apply with a missing target exits 1 and writes nothing"
else
    fail "apply with a missing target: expected exit 1 and no file changed, got $got: $out"
fi

victim="$(printf '%s' "$table" | jq -r '[.[] | select(.op == "replace-role")] | first | .id')"
r="$tmp/reworded-role"
build "$r" old
reword "$r" "$victim"
run "$r" --check
if [ "$got" = 1 ] && holds "missing: edit $victim, "; then
    echo "ok   --check names a reworded fs role as missing"
else
    fail "--check on a reworded fs role: expected exit 1 naming edit $victim missing, got $got: $out"
fi

# --- an old text that matches more than once ---
edit="$(printf '%s' "$table" | jq -c '[.[] | select(.op == "replace")] | last')"
victim="$(printf '%s' "$edit" | jq -r '.id')"
target_node="$(printf '%s' "$edit" | jq -r '.node')"
r="$tmp/two-elements"
build "$r" old
jq --argjson e "$edit" '.[$e.field] += ["Another element, " + $e.old + "."]' "$r/$target_node/NODE.inf.json" >"$tmp/doubled"
cat "$tmp/doubled" >"$r/$target_node/NODE.inf.json"
run "$r" --check
if [ "$got" = 1 ] && holds "missing: edit $victim, "; then
    echo "ok   --check names an old text held by two elements as missing"
else
    fail "--check on an old text in two elements: expected exit 1 naming edit $victim missing, got $got: $out"
fi
r="$tmp/twice-in-one"
build "$r" old
jq --argjson e "$edit" '.[$e.field] |= map(split($e.old) | join($e.old + " and " + $e.old))' \
    "$r/$target_node/NODE.inf.json" >"$tmp/doubled"
cat "$tmp/doubled" >"$r/$target_node/NODE.inf.json"
run "$r" --check
if [ "$got" = 1 ] && holds "missing: edit $victim, "; then
    echo "ok   --check names an old text held twice in one element as missing"
else
    fail "--check on an old text twice in one element: expected exit 1 naming edit $victim missing, got $got: $out"
fi

role_edit="$(printf '%s' "$table" | jq -c '[.[] | select(.op == "replace-role")] | first')"
role_id="$(printf '%s' "$role_edit" | jq -r '.id')"
role_node="$(printf '%s' "$role_edit" | jq -r '.node')"
r="$tmp/role-twice"
build "$r" old
jq --argjson e "$role_edit" '
    .fs |= map(if .name == $e.entry then .role |= (split($e.old) | join($e.old + " and " + $e.old)) else . end)
' "$r/$role_node/NODE.inf.json" >"$tmp/doubled"
cat "$tmp/doubled" >"$r/$role_node/NODE.inf.json"
run "$r" --check
if [ "$got" = 1 ] && holds "missing: edit $role_id, "; then
    echo "ok   --check names an old role text held twice in one role as missing"
else
    fail "--check on an old role text twice in one role: expected exit 1 naming edit $role_id missing, got $got: $out"
fi

# --- a node file that exists but cannot be read, and a node properly removed ---
broken="$(printf '%s' "$table" | jq -r '[.[] | select(.node != ".")] | first | .node')"
r="$tmp/unreadable"
build "$r" old
printf 'not json at all\n' >"$r/$broken/NODE.inf.json"
before="$(sums "$r")"
run "$r" --check
if [ "$got" = 2 ] && holds "nodes get $broken " "$r/$broken/NODE.inf.json: " " at line 1 column"; then
    echo "ok   --check on an unreadable node file exits 2 with nodes' own message"
else
    fail "--check on an unreadable node file: expected exit 2 and nodes' message, got $got: $out"
fi
run "$r"
if [ "$got" = 2 ] && holds "nodes get $broken " "$r/$broken/NODE.inf.json: " " at line 1 column" && [ ! -s "$tmp/sets" ] && [ "$(sums "$r")" = "$before" ]; then
    echo "ok   apply on an unreadable node file exits 2 and writes nothing"
else
    fail "apply on an unreadable node file: expected exit 2, nodes' message and no set call, got $got: $out"
fi

r="$tmp/removed"
build "$r" old
rm "$r/$broken/NODE.inf.json"
jq --arg name "$broken/" '.fs |= map(select(.name != $name))' "$r/NODE.inf.json" >"$tmp/dropped"
cat "$tmp/dropped" >"$r/NODE.inf.json"
"$real_nodes" --root "$r" fmt >/dev/null
if ! "$real_nodes" --root "$r" check >"$tmp/check.out" 2>&1; then
    fail "the tree without $broken is no valid tree: $(cat "$tmp/check.out")"
fi
run "$r" --check
gone=yes
for id in $(printf '%s' "$table" | jq -r --arg n "$broken" '.[] | select(.node == $n) | .id'); do
    holds "missing: edit $id, " || gone=no
done
if [ "$got" = 1 ] && [ "$gone" = yes ]; then
    echo "ok   --check names the edits of a properly removed node as missing"
else
    fail "--check on a removed node $broken: expected exit 1 naming its edits missing, got $got: $out"
fi

# --- an entry to add already there with another role ---
r="$tmp/conflict"
build "$r" old
add="$(printf '%s' "$table" | jq -c '[.[] | select(.op == "add-entry")] | first')"
add_id="$(printf '%s' "$add" | jq -r '.id')"
add_node="$(printf '%s' "$add" | jq -r '.node')"
add_name="$(printf '%s' "$add" | jq -r '.entry')"
jq --arg name "$add_name" '.fs += [{name: $name, node: false, role: "another role"}]' \
    "$r/$add_node/NODE.inf.json" >"$tmp/conflicted"
cat "$tmp/conflicted" >"$r/$add_node/NODE.inf.json"
"$real_nodes" --root "$r" fmt >/dev/null
run "$r" --check
if [ "$got" = 1 ] && holds "missing: edit $add_id, "; then
    echo "ok   --check reports an fs entry of the same name with another role"
else
    fail "--check on a conflicting entry: expected exit 1 naming edit $add_id missing, got $got: $out"
fi
run "$r"
role="$(jq -r --arg name "$add_name" '.fs[] | select(.name == $name) | .role' "$r/$add_node/NODE.inf.json")"
if [ "$got" = 1 ] && [ "$role" = "another role" ] && [ ! -s "$tmp/sets" ]; then
    echo "ok   apply leaves a conflicting fs entry as it is"
else
    fail "apply on a conflicting entry: expected exit 1 and the role kept, got $got and \"$role\": $out"
fi

# --- preconditions ---
r="$tmp/pre"
build "$r" old
case_path="$tmp/old-nodes:$PATH"
run "$r" --check
if [ "$got" = 2 ] && holds "nodes 0.6.0 is older than 0.7"; then
    echo "ok   an older nodes exits 2"
else
    fail "an older nodes: expected exit 2, got $got: $out"
fi
unset case_path

cp "$r/NODE.inf.json" "$r/NODE.json"
before="$(sums "$r")"
run "$r"
if [ "$got" = 2 ] && holds "still holds a NODE.json" && [ "$(sums "$r")" = "$before" ]; then
    echo "ok   a root NODE.json (a tree from before nodes 0.7) exits 2"
else
    fail "a root NODE.json: expected exit 2, got $got: $out"
fi
rm "$r/NODE.json"

mv "$r/NODE.inf.json" "$r/NODE.inf.json.away"
run "$r" --check
if [ "$got" = 2 ] && holds "holds no NODE.inf.json"; then
    echo "ok   no root NODE.inf.json exits 2"
else
    fail "no root NODE.inf.json: expected exit 2, got $got: $out"
fi
mv "$r/NODE.inf.json.away" "$r/NODE.inf.json"

run "$r" --frobnicate
if [ "$got" = 2 ] && holds "unknown argument"; then
    echo "ok   an unknown argument exits 2"
else
    fail "an unknown argument: expected exit 2, got $got: $out"
fi

if [ "$failures" -ne 0 ]; then
    echo "$failures failure(s)"
    exit 1
fi
echo "all nodes-0-7-edits tests passed"
