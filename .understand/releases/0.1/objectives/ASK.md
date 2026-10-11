# Ask: each team's objectives for 0.1

From the Orchestrator, for the Engineer, 2026-10-09.

**The Engineer asked every team to define its objectives for release 0.1.** Each team lead writes one page. The team's QA challenges it before the lead finalizes it. The Orchestrator then folds the nine pages into one summary for the Engineer.

## What 0.1 is

- **Stage:** 0.1 ships as `pre-alpha-v0.1.0`. Read `.understand/releases/stages.md` first. In pre-alpha only the Engineer checks the work, development is fast, breaking changes may happen, and the proto stays at `v1` through a breaking change.
- **Scope:** "Sign in with Bluesky and browse My Den". It is drafted and **not yet signed**. Read these Typst sources (plain text) in `.understand/releases/0.1/`:
  - `pdd.typ`: "Goals and non-goals", "What 0.1 delivers on the Release Board", "Features", "Success criteria".
  - `plan.typ`: "Work breakdown" (find the work packages your team owns), "Phases and milestones".
  - `tdd.typ`: your feature's sections, "One-way doors in 0.1", "Parked: not in 0.1".
- The MVP scope as a whole is the corpus page "Project MVP" (`~/code/zurfur-design/project/589826-*.md`).

## What to write

Write one file: `.understand/releases/0.1/objectives/<team>.md`, using the template below. Keep it to one screen above the QA sections.

- **Objectives:** at most five. Each is an outcome, not a task. Each has a "Done when" the Engineer could check by hand (a click, a command, a page to read). Each says which feature or work package it serves, **named in words** (e.g. "the den API routes"), not just as a number.
- **No 0.1 work for your team?** Then say so in one line, and give at most two objectives that 0.1 needs from you anyway (e.g. a boundary that must not be crossed, or a decision that must not be closed off). "None" is a valid answer.
- **Where the stages change the plan.** The plan was written before the stages existed. Say where pre-alpha changes your work. Examples: something treated as a one-way door that pre-alpha makes reversible, rigor that pre-alpha doesn't need, or rigor that must stay anyway because it guards security or the private/public data boundary. Security and data-boundary rigor never relaxes with the stage.
- **Not this release.** Anything you'd expect in 0.1 that the plan leaves out, or anything in it you'd cut, with the reason.
- **Questions:** each one carries your proposed answer, marked as such.

## Addendum: PDD corrections

The Engineer wants the Product Design Document updated with every lead's input. Kestrel wrote it alone, and no lead reviewed it. So each lead also reads `pdd.typ` in full and adds a `## PDD corrections` section, placed before the QA sections. List what is wrong, missing or overstated for your team. Give each correction the exact section title and your proposed wording. "None" is a valid answer. QA checks these corrections along with the objectives.

## Template

```markdown
# 0.1 objectives: <team>

Stage: pre-alpha-v0.1.0 · Lead: <name> · QA: <name>

## Objectives
1. **<outcome>.** Done when: <check>. Serves: <feature / work package, in words>.

## Where the stages change our plan
- …

## Not this release
- …

## Questions for the Engineer (each pre-answered)
- …

## QA challenges
## QA resolutions
```

## Rules

- **Read-only, apart from your one file.** Don't edit the PDD, TDD, plan, code, corpus or another team's file. Run no `jj` or `git` command that changes anything; the Orchestrator commits.
- **QA before you finalize** (CLAUDE.md, "Roles & decision authority"). Your QA reads your draft and writes `## QA challenges`. You answer each in `## QA resolutions`, and change the objectives where QA is right.
- **Domain forks are the Engineer's.** If an objective needs a new domain decision, write it as a question, pre-answered. Don't settle it.
- **Plain words.** No bare ids. When you cite a decision or work package, name it.
