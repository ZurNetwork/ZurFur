#!/bin/sh
# The node-text edits that came with the move to nodes 0.7 (node files became
# NODE.inf.json, every ref moved into the root NODE.refs.json), kept as
# targeted, idempotent transforms so they can be replayed. Temporary: delete it
# once the last branch cut before the migration has caught up.
#
# Each edit replaces one substring inside one element of one node field, adds
# one `fs` entry, or removes one exact element; the table is in edits() below.
# It reads each field with `nodes get`, transforms it with jq and writes it
# back with `nodes set`, one `set` per changed field. It never rewrites a field
# from a stored copy and never runs `nodes touch`, so a second run, on any day,
# changes nothing.
#
# Each edit is in one of three states:
#   applied  its new text is in place (tested first, so an edit is idempotent
#            even when its new text contains its old text);
#   pending  its old text is there, once, in exactly one element;
#   missing  neither: the target is gone or reworded, more than one element
#            matches, or an entry to add already exists with another role.
#
# Usage, from the checkout's root (it works on the repo above its own folder):
#   sh scripts/nodes-0-7-edits.sh                 apply every pending edit
#   sh scripts/nodes-0-7-edits.sh --check         change nothing; report
#   sh scripts/nodes-0-7-edits.sh --print-edits   print the edit table (JSON)
# Exit codes: 0 every edit applied (after the run); 1 `--check` found an edit
# not applied (one line each), or apply found a missing edit and wrote nothing;
# 2 a precondition failed (no current tree, no jq, no nodes 0.7 or later), a
# `nodes` write failed, or `nodes get` failed on a node whose file exists (a
# node whose file is gone is `missing`, not an error).
#
# Refresh, when `main` moves under the PR with node changes: in a merge change,
#   jj restore --from main@origin 'glob:**/NODE.json' 'glob:**/NODE.inf.json' NODE.refs.json
#   nodes migrate
#   sh scripts/nodes-0-7-edits.sh
# Never replay `set` commands, and never hand-merge node JSON.
#
# Catch-up, for a branch b cut before X (X is the PR's squash commit on
# `main`, X^ its parent, `X-` in jj):
#   1. merge X^ into b the usual way (`jj new b X-`), then the light gate.
#      Both sides are still in the old format here, so the pinned nodes 0.7
#      can't write a conflicted node file, and node JSON is never merged by
#      hand. So keep a scratch nodes 0.6.0, installed once, until the last
#      branch has caught up:
#        CARGO_HOME=<scratch> sh scripts/nodes-install.sh v0.6.0
#      In the merge change, first, for every node file `jj resolve --list`
#      names, note b's own change, then take X^'s side (`root-file:` keeps a
#      path such as `frontend/web/src/routes/(session)/NODE.json` from being
#      parsed as a fileset expression):
#        jj diff --from 'fork_point(b | X-)' --to b 'root-file:"<node file>"'   b's own change
#        jj restore --from X- 'root-file:"<node file>"'                          X^'s side
#      Only once no node file is conflicted, redo b's noted changes with the
#      scratch binary's `set` / `add-ref` (`<scratch>/bin/nodes --root <b's
#      workspace> ...`): a conflicted root NODE.json would keep it from
#      finding the tree. Then `<scratch>/bin/nodes --root <b's workspace>
#      check` passes;
#   2. `nodes migrate` on b, then commit it;
#   3. `jj new b X`, then
#      `jj restore --from b 'glob:**/NODE.inf.json' 'glob:**/NODE.json' NODE.refs.json`
#      (other files merge as usual);
#   4. in that merge change, `sh scripts/nodes-0-7-edits.sh`;
#   5. check: `sh scripts/nodes-0-7-edits.sh --check` is clean,
#      `jj diff --from <step 2> 'glob:**/NODE.inf.json' NODE.refs.json` shows
#      only the script's edits, `just nodes-check` passes, the gate passes;
#   6. merge the rest of `main` normally.
# A missing edit target makes `--check` fail: stop and tell the Orchestrator.
set -eu

me=nodes-0-7-edits

# The edit table. op: "replace" (a substring of the one element of a string
# array holding it), "replace-role" (a substring of an `fs` entry's role),
# "add-entry" (an `fs` entry, node: false), "remove" (an exact element).
edits() {
    cat <<'EDITS'
[
  {"id": "1a", "node": ".", "field": "notes", "op": "replace",
   "old": "those live in the nearest NODE.json under `refs`",
   "new": "those live in `NODE.refs.json`, under the nearest node's path"},
  {"id": "1b", "node": ".", "field": "notes", "op": "replace",
   "old": "moves to that NODE.json's `notes`",
   "new": "moves to that node's `notes`"},
  {"id": "2", "node": ".", "field": "notes", "op": "replace",
   "old": "NODE.json is what/where; CLAUDE.md is how to behave.",
   "new": "The nodes (`NODE.inf.json`, their refs in the root `NODE.refs.json`) are what/where; CLAUDE.md is how to behave."},
  {"id": "3", "node": "docs", "field": "conventions", "op": "replace",
   "old": "resolves every NODE.json ref against it",
   "new": "resolves every ref in `NODE.refs.json` against it"},
  {"id": "4", "node": "scripts", "field": "fs", "op": "replace-role", "entry": "nodes-fmt-hook.sh",
   "old": "nodes fmt on any NODE.json a model writes; fails soft without the binary",
   "new": "nodes fmt on any NODE.inf.json or NODE.refs.json a model writes, in that file's own tree; silent on a tree from before nodes 0.7, refuses a NODE.json in a current one; fails soft without the binary or with one older than 0.7 (tested by nodes-fmt-hook.test.sh)"},
  {"id": "5", "node": "scripts", "field": "fs", "op": "replace-role", "entry": "gate-light.sh",
   "old": "for restacks and fix rounds",
   "new": "for base merges and fix rounds"},
  {"id": "6", "node": "scripts", "field": "fs", "op": "add-entry", "entry": "nodes-tree-check.sh",
   "role": "the tree guards `just nodes-check` runs after `nodes check`: no NODE.json outside hidden and build folders, a node tree here, every NODE.inf.json and NODE.refs.json tracked; fails closed (tested by nodes-tree-check.test.sh)"},
  {"id": "7", "node": "scripts", "field": "fs", "op": "add-entry", "entry": "nodes-0-7-edits.sh",
   "role": "TEMPORARY: the node-text edits of the move to nodes 0.7, as idempotent get → set transforms; `--check` lists any not applied; a branch cut before the migration replays it when it catches up (tested by nodes-0-7-edits.test.sh)"},
  {"id": "8", "node": "backend/crates/application", "field": "notes", "op": "replace",
   "old": "See the per-module NODE.json notes.",
   "new": "See the per-module nodes' notes."},
  {"id": "9", "node": "backend/crates/application", "field": "notes", "op": "remove",
   "text": "See also: memory `feedback_work_by_module`."},
  {"id": "10", "node": "backend/crates/adapter-pg", "field": "notes", "op": "replace",
   "old": "Migration headers that say `See NODE.md (\"<file>\")` mean this node",
   "new": "Migration headers that say `See NODE.md (\"<file>\")` or \"this crate's NODE.json notes\" mean this node"},
  {"id": "11", "node": "backend/crates/adapter-pg", "field": "fs", "op": "replace-role", "entry": "queries/",
   "old": "Carries NO NODE.json of its own",
   "new": "Carries no node of its own"},
  {"id": "12", "node": "frontend/web/src/routes", "field": "conventions", "op": "replace",
   "old": "(see `src/NODE.json`)",
   "new": "(see the `src` node)"}
]
EDITS
}

# Input: a field's current value. $edits: the edits on this node and field, in
# table order. Output: {node, field, value (every pending edit applied),
# changed, states: [{order, id, state, line}]}. Each edit is judged on the
# value the edits before it left.
program='
def occurrences($s): if type == "string" then [indices($s)[]] | length else 0 end;
def holding($s): [.[] | select(type == "string" and occurrences($s) > 0)];
def place($e):
    if $e.op == "replace" or $e.op == "remove" then "\($e.node) › \($e.field)"
    else "\($e.node) › \($e.field) › \($e.entry)" end;
def target($e):
    if $e.op == "replace" then ", the element holding \"\($e.old)\""
    elif $e.op == "replace-role" then " › role, \"\($e.old)\""
    elif $e.op == "add-entry" then ", an entry with node: false and the role \"\($e.role)\""
    else ", the element \"\($e.text)\"" end;
def state($e):
    if type != "array" then "missing"
    elif $e.op == "replace" then
        (holding($e.new) | length) as $new
        | holding($e.old) as $old
        | if $new == 1 then "applied"
          elif $new == 0 and ($old | length) == 1 and ($old[0] | occurrences($e.old)) == 1 then "pending"
          else "missing" end
    elif $e.op == "replace-role" then
        [.[] | select(type == "object" and .name == $e.entry)] as $m
        | if ($m | length) != 1 or ($m[0].role | type) != "string" then "missing"
          elif ($m[0].role | occurrences($e.new)) > 0 then "applied"
          elif ($m[0].role | occurrences($e.old)) == 1 then "pending"
          else "missing" end
    elif $e.op == "add-entry" then
        [.[] | select(type == "object" and .name == $e.entry)] as $m
        | if ($m | length) == 0 then "pending"
          elif ($m | length) == 1 and $m[0].role == $e.role and $m[0].node == false then "applied"
          else "missing" end
    elif $e.op == "remove" then
        if any(.[]; . == $e.text) then "pending" else "applied" end
    else "missing" end;
def apply($e):
    if $e.op == "replace" then
        map(if type == "string" and occurrences($e.old) > 0 then split($e.old) | join($e.new) else . end)
    elif $e.op == "replace-role" then
        map(if type == "object" and .name == $e.entry then .role |= (split($e.old) | join($e.new)) else . end)
    elif $e.op == "add-entry" then
        . + [{name: $e.entry, node: false, role: $e.role}]
    else map(select(. != $e.text)) end;
reduce $edits[] as $e ({node: $node, field: $field, value: ., changed: false, states: []};
    (.value | state($e)) as $s
    | .states += [{order: $e.order, id: $e.id, state: $s,
        line: "\($s): edit \($e.id), \(place($e))\(target($e))"}]
    | if $s == "pending" then .value |= apply($e) | .changed = true else . end)
'

fail_closed() {
    echo "$me: $*" >&2
    exit 2
}

mode=apply
case "${1-}" in
    '') ;;
    --check) mode=check ;;
    --print-edits)
        edits
        exit 0
        ;;
    *) fail_closed "unknown argument \"$1\" (usage: $0 [--check | --print-edits])" ;;
esac
[ "$#" -le 1 ] || fail_closed "one argument at most (usage: $0 [--check | --print-edits])"

root="$(cd "$(dirname "$0")/.." && pwd)" || fail_closed "could not resolve the checkout's root from $0"
command -v jq >/dev/null 2>&1 || fail_closed "\`jq\` is not on PATH"
command -v nodes >/dev/null 2>&1 || fail_closed "\`nodes\` is not on PATH (nodes 0.7 or later; just nodes-install)"
have="$(nodes --version 2>/dev/null | awk 'NR == 1 { print $2 }')" || have=""
current="$(printf '%s\n' "$have" | awk -F. '
    $1 ~ /^[0-9]+$/ && $2 ~ /^[0-9]+/ { print ($1 > 0 || $2 + 0 >= 7) ? "yes" : "no"; exit }
    { print "no" }')" || fail_closed "could not compare the version of nodes (awk failed)"
[ "$current" = yes ] || fail_closed "nodes ${have:-of an unknown version} is older than 0.7 (just nodes-install)"
[ -f "$root/NODE.inf.json" ] || fail_closed "$root holds no NODE.inf.json: not a tree of nodes 0.7 or later (run nodes migrate first)"
[ ! -f "$root/NODE.json" ] || fail_closed "$root still holds a NODE.json: the tree is from before nodes 0.7 (run nodes migrate first)"

table="$(edits | jq -c 'to_entries | map(.value + {order: .key})')" || fail_closed "the edit table does not parse"
work="$(mktemp -d)" || fail_closed "could not make a scratch folder (mktemp failed)"
trap 'rm -rf "$work"' EXIT

# evaluate: writes $work/results, one JSON object per node and field.
evaluate() {
    : >"$work/results"
    printf '%s' "$table" | jq -r '
        reduce .[] as $e ([]; if any(.[]; .[0] == $e.node and .[1] == $e.field) then . else . + [[$e.node, $e.field]] end)
        | .[] | @tsv' >"$work/pairs" || fail_closed "could not list the node and field pairs (jq failed)"
    tab="$(printf '\t')"
    while IFS="$tab" read -r node field; do
        mine="$(printf '%s' "$table" | jq -c --arg n "$node" --arg f "$field" 'map(select(.node == $n and .field == $f))')" ||
            fail_closed "could not pick the edits of $node › $field (jq failed)"
        if ! value="$(nodes --root "$root" --json get "$node" "$field" 2>"$work/err")"; then
            # A node whose file is gone has its edits missing; one whose file
            # exists but cannot be read is a tool failure, never a guess.
            file="$root/$node/NODE.inf.json"
            [ "$node" != . ] || file="$root/NODE.inf.json"
            if [ -e "$file" ]; then
                fail_closed "nodes get $node $field failed on an existing node file: $(cat "$work/err")"
            fi
            value='"(no such node or field)"'
        fi
        printf '%s' "$value" |
            jq -c --argjson edits "$mine" --arg node "$node" --arg field "$field" "$program" >>"$work/results" ||
            fail_closed "could not read $node › $field (nodes get printed no JSON)"
    done <"$work/pairs"
}

# lines <state filter>: the report lines of the edits whose state matches.
lines() {
    jq -rs --arg want "$1" --arg me "$me" '[.[] | .states[]] | sort_by(.order) | .[]
        | select(if $want == "not-applied" then .state != "applied" else .state == $want end)
        | $me + ": " + .line' "$work/results"
}

total="$(printf '%s' "$table" | jq 'length')" || fail_closed "could not count the edits (jq failed)"
evaluate

if [ "$mode" = check ]; then
    report="$(lines not-applied)" || fail_closed "could not list the edits not applied (jq failed)"
    if [ -n "$report" ]; then
        printf '%s\n' "$report"
        exit 1
    fi
    echo "$me: OK, all $total edits applied"
    exit 0
fi

missing="$(lines missing)" || fail_closed "could not list the missing edits (jq failed)"
if [ -n "$missing" ]; then
    printf '%s\n' "$missing"
    echo "$me: nothing written: a missing edit needs a person (tell the Orchestrator)"
    exit 1
fi

pending="$(jq -s '[.[] | .states[] | select(.state == "pending")] | length' "$work/results")" ||
    fail_closed "could not count the pending edits (jq failed)"
jq -c 'select(.changed)' "$work/results" >"$work/changed" ||
    fail_closed "could not pick the changed fields (jq failed); nothing written"
sets=0
while IFS= read -r result; do
    node="$(printf '%s' "$result" | jq -r '.node')" ||
        fail_closed "could not read a node from a result (jq failed; $sets field(s) written before it)"
    field="$(printf '%s' "$result" | jq -r '.field')" ||
        fail_closed "could not read the field of $node from a result (jq failed; $sets field(s) written before it)"
    value="$(printf '%s' "$result" | jq -c '.value')" ||
        fail_closed "could not read the value of $node › $field from a result (jq failed; $sets field(s) written before it)"
    nodes --root "$root" set "$node" "$field" "$value" ||
        fail_closed "nodes set $node $field failed ($sets field(s) written before it; rerun once fixed)"
    sets=$((sets + 1))
done <"$work/changed"

evaluate
report="$(lines not-applied)" || fail_closed "could not list the edits not applied after writing (jq failed)"
if [ -n "$report" ]; then
    printf '%s\n' "$report"
    echo "$me: an edit is still not applied after writing; tell the Orchestrator"
    exit 1
fi
echo "$me: OK, all $total edits applied ($pending pending edit(s) written with $sets nodes set call(s))"
