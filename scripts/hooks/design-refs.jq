# jq filter: build the "Governs this file" / "Other pages cited" lines for
# design-refs.sh from a `nodes chain --json` array. Args: $basename (the
# touched file's basename) and $index (design-index.md rows, [{id,dir,
# status,title}]). Input: the chain array (root..leaf). Every free-text
# field (title, governs, a node's `short`) is untrusted — someone else's
# NODE.json — so it is neutralized (control chars stripped, newlines
# collapsed, '<'/'>' removed) before it is truncated and formatted.

def neutralize:
  gsub("[\r\n\t]"; " ")
  | gsub("[[:cntrl:]]"; "")
  | gsub("[<>]"; "");

def trunc(s; n):
  (s | neutralize) as $s
  | if ($s | length) > n then ($s[0:(n - 3)] + "...") else $s end;

def basename_matches(g):
  ([g | splits("[ ,;]+")]) as $toks
  | any($toks[]; . == $basename or endswith("/" + $basename));

($index | map({key: (.id | tostring), value: .}) | from_entries) as $idx
# Flatten every ref on the chain, leaf to root, each tagged with whether
# THAT occurrence's governs names this file.
| [
    (reverse)[] as $node
    | ($node.refs // [])[]
    | {
        id: (.page | tostring),
        title: .title,
        governs: (.governs // ""),
        matches: basename_matches(.governs // "")
      }
  ] as $flat
# One record per id, first-appearance (leaf-first) order kept. An id is
# "governs" if ANY occurrence anywhere on the chain matches — that
# occurrence's text wins even when a closer, non-matching occurrence was
# seen first. A ref an id already earned as "governs" never falls back to
# "other" from a farther, non-matching occurrence.
| (
    reduce $flat[] as $r ({order: [], info: {}};
      $r.id as $id
      | if (.info[$id] == null) then
          .order += [$id] | .info[$id] = {title: $r.title, governs: $r.governs, matches: $r.matches}
        elif ((.info[$id].matches | not) and $r.matches) then
          .info[$id] = {title: $r.title, governs: $r.governs, matches: true}
        else . end
    )
  ) as $collected
| {
    leaf_path: .[-1].path,
    leaf_short: (.[-1].short // "" | trunc(.; 200)),
    governs_total: ($collected.order | map(select($collected.info[.].matches)) | length),
    other_total: ($collected.order | map(select($collected.info[.].matches | not)) | length),
    governs_lines: [
      $collected.order[] as $id
      | $collected.info[$id] | select(.matches)
      | ($id + " " + trunc(.title; 70) + " — governs: " + trunc(.governs; 120))
    ],
    other_lines: [
      $collected.order[] as $id
      | $collected.info[$id] | select(.matches | not)
      | $idx[$id] as $e
      | (if $e == null then "NOT-IN-INDEX"
         elif $e.status == "SUPERSEDED" then "SUPERSEDED"
         else $e.dir end) as $dircol
      | ($id + " " + $dircol + " " + trunc(.title; 70))
    ]
  }
