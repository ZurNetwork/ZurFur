// Shared template for the Zurfur release 0.1 document set.
// Light theme, A4. Every document starts with:
//   #import "template.typ": *
//   #show: doc.with(title: "...", short: "...", status: "Draft", ...)
//
// Diagrams use fletcher 0.5.8, the latest on Typst Universe as of 2026-10-08
// (released 2025-05-27, built for Typst 0.13, depends on cetz 0.3.4). pacman
// offers Typst 0.15.1. A read of fletcher 0.5.8 and cetz 0.3.4 found none of
// the features Typst 0.14 and 0.15 removed (the `path` element, `pattern`,
// the removed symbols, type-to-string comparisons); only a compile proves it.

#import "@preview/fletcher:0.5.8" as fletcher: diagram, node, edge

// ---------------------------------------------------------------- palette

#let colors = (
  ink: rgb("#1f2328"),
  muted: rgb("#59636e"),
  rule: rgb("#d1d9e0"),
  faint: rgb("#f6f8fa"),
  accent: rgb("#2a5d8f"),
  accent-soft: rgb("#eaf1f8"),
  decision: rgb("#8a5a00"),
  decision-soft: rgb("#fff8e5"),
  door: rgb("#b42318"),
  door-soft: rgb("#fdf0ee"),
  ok: rgb("#1a7f37"),
  ok-soft: rgb("#e9f6ec"),
  warn: rgb("#8a5a00"),
  warn-soft: rgb("#fff4d6"),
  bad: rgb("#b42318"),
  bad-soft: rgb("#fdecea"),
  critical: rgb("#c2410c"),
  critical-soft: rgb("#fff1e8"),
  // Team colours, as in the approved My Den architecture diagrams.
  web: rgb("#2f6fb5"),
  den: rgb("#7a4fb5"),
  identity: rgb("#2e8b57"),
  commission: rgb("#c0662b"),
  shared: rgb("#777777"),
)

#let fonts = (
  body: ("Noto Sans", "Liberation Sans", "DejaVu Sans"),
  mono: ("DejaVu Sans Mono", "Fira Code"),
)

// ---------------------------------------------------------------- small parts

// A rounded label, used for ids, statuses and levels.
#let pill(body, fg: colors.accent, bg: colors.accent-soft) = box(
  fill: bg,
  stroke: 0.5pt + fg,
  radius: 3pt,
  inset: (x: 4pt, y: 1.5pt),
  baseline: 1.5pt,
  text(fill: fg, size: 0.8em, weight: "semibold", body),
)

// L / M / H chip for likelihood and impact.
#let chip(l) = {
  let pair = if l == "H" {
    (colors.bad, colors.bad-soft)
  } else if l == "M" {
    (colors.warn, colors.warn-soft)
  } else {
    (colors.ok, colors.ok-soft)
  }
  pill(l, fg: pair.at(0), bg: pair.at(1))
}

// S / M / L size chip for work packages.
#let size(s) = pill(s, fg: colors.ink, bg: colors.faint)

#let meta-label(body) = text(size: 8.5pt, fill: colors.muted, weight: "semibold", body)

#let status-colors = (
  "Draft": (colors.muted, colors.faint),
  "In review": (colors.warn, colors.warn-soft),
  "Signed": (colors.ok, colors.ok-soft),
)

// Draft -> In review -> Signed, with the current step highlighted.
#let status-track(status) = {
  let steps = ("Draft", "In review", "Signed")
  steps
    .map(s => if s == status {
      let pair = status-colors.at(s)
      pill(s, fg: pair.at(0), bg: pair.at(1))
    } else {
      text(fill: colors.muted, size: 8.5pt, s)
    })
    .join(text(fill: colors.muted, size: 8.5pt)[ #sym.arrow.r ])
}

// ---------------------------------------------------------------- the document

#let doc(
  title: "",
  short: "",
  subtitle: none,
  release: "0.1",
  version: "1",
  date: "2026-10-08",
  status: "Draft",
  author: "",
  approver: "",
  body,
) = {
  set document(title: title + " · Zurfur release " + release, author: author)
  set page(
    paper: "a4",
    margin: (x: 2.1cm, top: 2.3cm, bottom: 2.2cm),
    header: context {
      if counter(page).get().first() > 1 {
        set text(size: 8pt, fill: colors.muted)
        grid(
          columns: (1fr, auto),
          [Zurfur · Release #release · #short], [#status · version #version · #date],
        )
        v(-3pt)
        line(length: 100%, stroke: 0.4pt + colors.rule)
      }
    },
    footer: context {
      set text(size: 8pt, fill: colors.muted)
      grid(
        columns: (1fr, auto),
        [#title], counter(page).display("1 / 1", both: true),
      )
    },
  )
  set text(font: fonts.body, size: 10pt, fill: colors.ink, lang: "en")
  set par(justify: false, leading: 0.62em, spacing: 0.95em)
  set heading(numbering: "1.1")
  show heading: set text(fill: colors.accent)
  show heading.where(level: 1): set text(size: 15pt)
  show heading.where(level: 2): set text(size: 12pt)
  show heading.where(level: 3): set text(size: 10.5pt)
  show heading: set block(above: 1.5em, below: 0.75em)
  show link: set text(fill: colors.accent)
  set list(indent: 0.4em, marker: ([•], [–]))
  set enum(indent: 0.4em)
  show raw: set text(font: fonts.mono, size: 0.9em)
  show raw.where(block: false): box.with(
    fill: colors.faint,
    inset: (x: 2pt, y: 0pt),
    outset: (y: 2pt),
    radius: 2pt,
  )
  show raw.where(block: true): block.with(
    fill: colors.faint,
    stroke: 0.5pt + colors.rule,
    inset: 8pt,
    radius: 3pt,
    width: 100%,
  )
  set table(
    stroke: 0.5pt + colors.rule,
    inset: (x: 5pt, y: 4pt),
    fill: (x, y) => if y == 0 { colors.faint },
  )
  show table: set text(size: 8.8pt)
  show table.cell.where(y: 0): set text(weight: "bold")
  show figure.caption: set text(size: 8.8pt, fill: colors.muted)

  // Title block.
  block(width: 100%, inset: (bottom: 9pt), stroke: (bottom: 1.2pt + colors.accent), {
    text(size: 8.5pt, fill: colors.muted, weight: "semibold", tracking: 0.06em, upper("Zurfur · Release " + release))
    v(1pt)
    block(below: 4pt, text(size: 22pt, weight: "bold", fill: colors.ink, title))
    if subtitle != none {
      text(size: 11pt, fill: colors.muted, subtitle)
    }
  })
  v(4pt)
  let pair = status-colors.at(status, default: (colors.muted, colors.faint))
  grid(
    columns: (auto, 1fr, auto, 1fr),
    column-gutter: 10pt,
    row-gutter: 6pt,
    meta-label[Release], [#release],
    meta-label[Document version], [#version],
    meta-label[Date], [#date],
    meta-label[Status], pill(status, fg: pair.at(0), bg: pair.at(1)),
    meta-label[Prepared by], [#author],
    meta-label[Approver], [#approver],
  )
  v(3pt)
  grid(columns: (auto, 1fr), column-gutter: 10pt, meta-label[Workflow], status-track(status))
  v(10pt)
  body
}

// ---------------------------------------------------------------- blocks

// The "In one page" summary box.
#let summary(title: "In one page", body) = block(
  width: 100%,
  fill: colors.accent-soft,
  stroke: (left: 3pt + colors.accent),
  inset: (x: 12pt, y: 10pt),
  radius: (right: 3pt),
  {
    text(weight: "bold", fill: colors.accent, size: 11pt, title)
    v(3pt)
    body
  },
)

// A short callout.
#let note(title: "Note", kind: "info", body) = {
  let pair = if kind == "warn" {
    (colors.warn, colors.warn-soft)
  } else if kind == "bad" {
    (colors.bad, colors.bad-soft)
  } else if kind == "ok" {
    (colors.ok, colors.ok-soft)
  } else {
    (colors.accent, colors.accent-soft)
  }
  block(
    width: 100%,
    fill: pair.at(1),
    stroke: 0.6pt + pair.at(0),
    inset: (x: 9pt, y: 7pt),
    radius: 3pt,
    [#text(weight: "bold", fill: pair.at(0), title) #h(4pt) #body],
  )
}

// A decision for the Engineer.
//   door: true marks a one-way door, which waits for the Engineer's review.
//   approval: true marks a two-way decision that still waits for the Engineer's approval
//   (for example, a change to an approved diagram).
//   options: an array of content, rendered (a), (b), (c) ...
#let decision(
  id: "D-?",
  title: [],
  question: none,
  options: (),
  why: none,
  answer: [],
  door: false,
  approval: false,
  adjust-by: none,
  source: none,
) = {
  let fg = if door { colors.door } else { colors.decision }
  let bg = if door { colors.door-soft } else { colors.decision-soft }
  let waits = door or approval
  let badge = if door {
    pill([One-way door · awaiting your review], fg: colors.door, bg: white)
  } else if approval {
    pill([Awaiting your approval], fg: colors.decision, bg: white)
  } else {
    pill([Work proceeds on Claude's answer], fg: colors.ok, bg: white)
  }
  block(width: 100%, stroke: 0.8pt + fg, radius: 3pt, inset: 0pt, above: 1.1em, below: 1.1em, {
    block(width: 100%, fill: bg, inset: (x: 10pt, y: 6pt), below: 0pt, radius: (top: 3pt), grid(
      columns: (auto, 1fr, auto),
      column-gutter: 8pt,
      align: horizon,
      pill(text(size: 1.1em, id), fg: fg, bg: white), text(weight: "bold", title), badge,
    ))
    block(width: 100%, inset: (x: 10pt, top: 7pt, bottom: 9pt), {
      if question != none {
        block(below: 6pt)[#text(weight: "semibold")[Question.] #question]
      }
      if options.len() > 0 {
        block(below: 2pt, text(weight: "semibold")[Options])
        enum(numbering: "(a)", tight: true, ..options)
      }
      if why != none {
        block(above: 6pt)[#text(weight: "semibold")[Recommendation and why.] #why]
      }
      block(
        above: 8pt,
        width: 100%,
        fill: white,
        stroke: 0.7pt + fg,
        inset: 7pt,
        radius: 2pt,
      )[#text(weight: "bold", fill: fg)[Answer (Claude's; adjust if wrong):] #answer]
      v(5pt)
      grid(
        columns: (auto, 1fr),
        column-gutter: 6pt,
        align: bottom,
        if waits {
          text(weight: "semibold")[Your review (required before dependent work starts):]
        } else {
          text(weight: "semibold")[Your adjustment, if any:]
        },
        box(width: 100%, height: 1.1em, stroke: (bottom: 0.5pt + colors.muted)),
      )
      if adjust-by != none or source != none {
        let by-label = if waits { [Needed by] } else { [Adjust by] }
        v(4pt)
        set text(size: 8.5pt, fill: colors.muted)
        if adjust-by != none {
          block(below: 2pt)[*#by-label:* #adjust-by]
        }
        if source != none {
          block(below: 0pt)[*Source:* #source]
        }
      }
    })
  })
}

// A checkable acceptance-criteria list. Items are numbered <prefix>.1, .2 ...
#let criteria(prefix: "AC", ..items) = {
  let items = items.pos()
  block(inset: (left: 2pt), above: 5pt, below: 4pt, {
    for (i, it) in items.enumerate(start: 1) {
      block(below: 4pt, grid(
        columns: (10pt, 46pt, 1fr),
        column-gutter: 4pt,
        box(width: 7.5pt, height: 7.5pt, stroke: 0.7pt + colors.muted, radius: 1pt),
        text(font: fonts.mono, size: 7.5pt, fill: colors.muted, prefix + "." + str(i)),
        it,
      ))
    }
  })
}

// A user story with its acceptance criteria.
#let story(id: "", title: [], who: [], want: [], why: [], criteria-items: ()) = {
  block(width: 100%, above: 1.1em, below: 0.9em, {
    grid(
      columns: (auto, 1fr),
      column-gutter: 8pt,
      align: horizon,
      pill(id), text(weight: "bold", title),
    )
    block(above: 5pt, below: 3pt, text(style: "italic", [As ] + who + [, I want ] + want + [, so that ] + why + [.]))
    criteria(prefix: "AC-" + id.trim("US-", at: start), ..criteria-items)
  })
}

// The risk-table style. Each row: (id, risk, likelihood, impact, mitigation, cut-first).
#let risk-table(..rows) = table(
  columns: (auto, 2.3fr, auto, auto, 2.5fr, 1.7fr),
  align: (left, left, center, center, left, left),
  table.header([\#], [Risk], [Likelihood], [Impact], [Mitigation], [Cut first, or fallback]),
  ..rows.pos().map(r => (r.at(0), r.at(1), chip(r.at(2)), chip(r.at(3)), r.at(4), r.at(5))).flatten(),
)

// A signature line.
#let signoff(..people) = block(breakable: false, grid(
  columns: (1fr,) * people.pos().len(),
  column-gutter: 18pt,
  ..people.pos().map(p => block(width: 100%, {
    v(26pt)
    line(length: 100%, stroke: 0.6pt + colors.ink)
    v(2pt)
    text(weight: "semibold", p.at(0))
    linebreak()
    text(size: 8.5pt, fill: colors.muted, p.at(1))
    v(10pt)
    grid(
      columns: (auto, 1fr),
      column-gutter: 6pt,
      text(size: 8.5pt, fill: colors.muted)[Date],
      box(width: 100%, height: 1em, stroke: (bottom: 0.5pt + colors.muted)),
    )
  })),
))

// ---------------------------------------------------------------- diagrams

// A box for an architecture diagram, outlined in a team colour.
#let arch-node(pos, body, team: colors.shared, dashed: false, width: auto, name: none) = node(
  pos,
  align(center, text(size: 7.5pt, body)),
  name: name,
  width: width,
  inset: 5pt,
  fill: white,
  stroke: (paint: team, thickness: 1pt, dash: if dashed { "dashed" } else { none }),
  corner-radius: 3pt,
)

// A sequence diagram drawn with fletcher.
//   actors: array of content (column headers, left to right)
//   steps:  array of dictionaries (from: int, to: int, label: content, kind: "call" | "reply" | "refuse")
//           a step whose from == to is drawn as a note on that lifeline.
#let seq(actors, steps, col: 24mm, gap: 4mm, row: 7.2mm) = {
  let n = steps.len()
  diagram(
    spacing: (gap, row),
    edge-stroke: 0.7pt + colors.ink,
    {
      for (i, a) in actors.enumerate() {
        node(
          (i, 0),
          align(center, text(size: 7.3pt, weight: "bold", a)),
          width: col,
          inset: 4pt,
          fill: colors.accent-soft,
          stroke: 0.7pt + colors.accent,
          corner-radius: 2pt,
        )
        edge((i, 0), (i, n + 1), stroke: 0.6pt + colors.rule, dash: "dashed")
      }
      for (k, s) in steps.enumerate(start: 1) {
        let kind = s.at("kind", default: "call")
        if s.from == s.to {
          node(
            (s.from, k),
            align(left, text(size: 6.6pt, s.label)),
            width: col,
            inset: 3pt,
            fill: colors.warn-soft,
            stroke: 0.5pt + colors.warn,
            corner-radius: 1.5pt,
          )
        } else {
          let paint = if kind == "refuse" { colors.bad } else { colors.ink }
          let marks = if kind == "reply" { "--|>" } else { "-|>" }
          edge(
            (s.from, k),
            (s.to, k),
            marks: marks,
            stroke: 0.7pt + paint,
            label: text(size: 6.6pt, fill: paint, s.label),
            label-side: center,
          )
        }
      }
    },
  )
}
